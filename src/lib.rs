use anyhow::Error;
use csv::{ReaderBuilder, Trim, WriterBuilder};
use fixed::traits::ToFixed;
use fixed::types::I50F14;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Eq, PartialEq)]
struct Account {
    client: u16,
    available: I50F14,
    held: I50F14,
    total: I50F14,
    locked: bool,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Clone)]
struct Transaction {
    #[serde(rename = "type")]
    tx_type: TransactionType,
    client: u16,
    #[serde(rename = "tx")]
    id: u32,
    amount: Option<I50F14>,
    #[serde(default)]
    under_dispute: bool,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Clone)]
enum TransactionType {
    #[serde(alias = "deposit")]
    Deposit,
    #[serde(alias = "withdraw")]
    Withdraw,
    #[serde(alias = "dispute")]
    Dispute,
    #[serde(alias = "resolve")]
    Resolve,
    #[serde(alias = "chargeback")]
    Chargeback,
}

pub fn run(input: &str, verbose: bool) -> Result<(), Error> {
    let mut reader = ReaderBuilder::new()
        .flexible(true)
        .trim(Trim::All)
        .from_path(input)?;
    let mut history: Vec<Transaction> = Vec::new();
    let mut accounts: Vec<Account> = Vec::new();

    for result in reader.deserialize() {
        use TransactionType::*;

        let record: Transaction = result?;
        history.push(record.clone());

        let res = match record.tx_type {
            Deposit => deposit(&mut accounts, record),
            Withdraw => withdraw(&mut accounts, record),
            Dispute => dispute(&mut accounts, record, &mut history),
            Resolve => resolve(&mut accounts, record, &mut history),
            Chargeback => chargeback(&mut accounts, record, &mut history),
        };

        if let Err(err) = res {
            if verbose {
                println!("{:?}; Error: {}", history.last().unwrap(), err);
            }
        };
    }

    write_output(accounts)?;

    Ok(())
}

fn write_output(accounts: Vec<Account>) -> Result<(), Error> {
    let mut writer = WriterBuilder::new().from_writer(std::io::stdout());

    for account in accounts {
        writer.serialize(account)?;
    }

    writer.flush()?;

    Ok(())
}

/// A deposit is a credit to the client’s asset account. It increases the available and total funds of the client account
/// by the transaction amount
fn deposit(accounts: &mut Vec<Account>, tx: Transaction) -> Result<(), Error> {
    let amount = tx.amount.ok_or(Error::msg("Deposit amount required"))?;
    match accounts.iter_mut().find(|item| item.client == tx.client) {
        Some(account) => {
            account.available = account.available + amount;
            account.total = account.total + amount;
        }
        None => {
            accounts.push(Account {
                client: tx.client,
                available: amount,
                held: 0.to_fixed(),
                total: amount,
                locked: false,
            });
        }
    };

    Ok(())
}

/// A withdraw is a debit to the client’s asset account. It decreases the available and total funds of the client account
/// by the transaction amount. If a client does not have sufficient available funds the withdraw will fail and the total
/// amount of funds will not change
fn withdraw(accounts: &mut Vec<Account>, tx: Transaction) -> Result<(), Error> {
    let amount = tx.amount.ok_or(Error::msg("Deposit amount required"))?;
    let account = accounts
        .iter_mut()
        .find(|item| item.client == tx.client)
        .ok_or(Error::msg("Account not found"))?;

    if amount <= account.available {
        account.available = account.available - amount;
        account.total = account.total - amount;
        Ok(())
    } else {
        Err(Error::msg("Insufficient funds for withdraw"))
    }
}

/// A dispute represents a claim that a transaction was erroneous and should be reversed. The transaction is not immediately
/// reversed; instead, the disputed amount is moved from available to held. The account total does not change.
///
/// Both deposits and withdrawals can be disputed. The latter case would apply in a scenario such as a stolen ATM card being
/// used to make a fraudulent withdrawal.
///
/// Disputes do not specify an amount. Instead they refer to a transaction by ID. If the transaction specified doesn’t exist,
/// the dispute is ignored.
fn dispute(
    accounts: &mut Vec<Account>,
    tx: Transaction,
    history: &mut Vec<Transaction>,
) -> Result<(), Error> {
    let disputed_tx = history
        .iter_mut()
        .find(|item| item.id == tx.id)
        .ok_or(Error::msg("Disputed transaction not found"))?;
    let disputed_amount = disputed_tx.amount.ok_or(Error::msg(
        "Disputed transaction does not have a valid amount",
    ))?;

    if disputed_tx.under_dispute {
        return Err(Error::msg("Transactoin already under dispute"));
    }

    let account = accounts
        .iter_mut()
        .find(|item| item.client == tx.client && item.client == disputed_tx.client) // the dispute and disputed transaction should both should have the same client id
        .ok_or(Error::msg("Account not found"))?;

    match disputed_tx.tx_type {
        TransactionType::Deposit => {
            account.available = account.available - disputed_amount;
            account.held = account.held + disputed_amount;
        }
        TransactionType::Withdraw => {
            account.held = account.held + disputed_amount;
            account.total = account.total + disputed_amount;
        }
        _ => return Err(Error::msg("Cannot dispute this type of transaction")),
    };

    disputed_tx.under_dispute = true;

    Ok(())
}

/// A resolve represents a resolution to a dispute, releasing the associated held funds. Funds that were previously disputed are
/// no longer disputed. The clients held funds decrease by the amount no longer disputed, their available funds increase by the amount
///  no longer disputed, and their total funds remain the same.
///
/// Resolves do not specify an amount. Instead they refer to a disputed transaction by ID. If the transaction specified doesn’t exist,
/// or the transaction isn’t under dispute, the resolve is ignored.
fn resolve(
    accounts: &mut Vec<Account>,
    tx: Transaction,
    history: &mut Vec<Transaction>,
) -> Result<(), Error> {
    let disputed_tx = history
        .iter_mut()
        .find(|item| item.id == tx.id)
        .ok_or(Error::msg("Disputed transaction not found"))?;
    let disputed_amount = disputed_tx.amount.ok_or(Error::msg(
        "Disputed transaction does not have a valid amount",
    ))?;

    if !disputed_tx.under_dispute {
        return Err(Error::msg("Cannot resolve transaction not under dispute"));
    }

    let account = accounts
        .iter_mut()
        .find(|item| item.client == tx.client && item.client == disputed_tx.client) // the dispute and disputed transaction should both should have the same client id
        .ok_or(Error::msg("Account not found"))?;

    match disputed_tx.tx_type {
        TransactionType::Deposit => {
            account.available = account.available + disputed_amount;
            account.held = account.held - disputed_amount;
        }
        TransactionType::Withdraw => {
            account.held = account.held - disputed_amount;
            account.available = account.available + disputed_amount;
        }
        _ => return Err(Error::msg("Cannot resolve this type of transaction")),
    };

    disputed_tx.under_dispute = false;

    Ok(())
}

/// A chargeback is the final state of a dispute and represents the client reversing a transaction. Funds that were held are now withdrawn.
/// The clients held funds and total funds decrease by the amount previously disputed. The client account is also frozen.
fn chargeback(
    accounts: &mut Vec<Account>,
    tx: Transaction,
    history: &mut Vec<Transaction>,
) -> Result<(), Error> {
    let disputed_tx = history
        .iter_mut()
        .find(|item| item.id == tx.id)
        .ok_or(Error::msg("Disputed transaction not found"))?;
    let disputed_amount = disputed_tx.amount.ok_or(Error::msg(
        "Disputed transaction does not have a valid amount",
    ))?;

    if !disputed_tx.under_dispute {
        return Err(Error::msg(
            "Cannot chargeback transaction not under dispute",
        ));
    }

    let account = accounts
        .iter_mut()
        .find(|item| item.client == tx.client && item.client == disputed_tx.client) // the dispute and disputed transaction should both should have the same client id
        .ok_or(Error::msg("Account not found"))?;

    match disputed_tx.tx_type {
        TransactionType::Deposit => {
            account.held = account.held - disputed_amount;
            account.total = account.total - disputed_amount;
            account.locked = true;
        }
        TransactionType::Withdraw => {
            account.held = account.held - disputed_amount;
            account.total = account.total - disputed_amount;
            account.locked = true;
        }
        _ => return Err(Error::msg("Cannot chargeback this type of transaction")),
    };

    disputed_tx.under_dispute = false;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deposit_adds_to_account() {
        let mut accounts = vec![Account {
            client: 1,
            available: 0.to_fixed(),
            held: 0.to_fixed(),
            total: 0.to_fixed(),
            locked: false,
        }];

        deposit(
            &mut accounts,
            Transaction {
                tx_type: TransactionType::Deposit,
                client: 1,
                id: 1,
                amount: Some(1.9999.to_fixed()),
                under_dispute: false,
            },
        )
        .unwrap();

        assert_eq!(
            accounts.get(0).unwrap().available,
            1.9999.to_fixed::<I50F14>()
        );
        assert_eq!(accounts.get(0).unwrap().total, 1.9999.to_fixed::<I50F14>());
    }

    #[test]
    fn withdraw_takes_from_account() {
        let mut accounts = vec![Account {
            client: 0,
            available: 2.to_fixed(),
            held: 0.to_fixed(),
            total: 2.to_fixed(),
            locked: false,
        }];

        withdraw(
            &mut accounts,
            Transaction {
                tx_type: TransactionType::Withdraw,
                client: 0,
                id: 1,
                amount: Some(1.9999.to_fixed()),
                under_dispute: false,
            },
        )
        .unwrap();

        assert_eq!(
            accounts.get(0).unwrap().available,
            0.0001.to_fixed::<I50F14>()
        );
        assert_eq!(accounts.get(0).unwrap().total, 0.0001.to_fixed::<I50F14>());
    }

    #[test]
    fn withdraw_fails_on_insufficient_funds() {
        let mut accounts = vec![Account {
            client: 0,
            available: 1.to_fixed(),
            held: 0.to_fixed(),
            total: 1.to_fixed(),
            locked: false,
        }];

        let res = withdraw(
            &mut accounts,
            Transaction {
                tx_type: TransactionType::Withdraw,
                client: 0,
                id: 1,
                amount: Some(1.9999.to_fixed()),
                under_dispute: false,
            },
        );

        assert!(res.is_err());
    }

    #[test]
    fn disputed_amount_should_move_to_held() {
        let mut accounts = vec![Account {
            client: 0,
            available: 1.to_fixed(),
            held: 0.to_fixed(),
            total: 1.to_fixed(),
            locked: false,
        }];

        let mut history = vec![Transaction {
            tx_type: TransactionType::Deposit,
            client: 0,
            id: 1,
            amount: Some(1.to_fixed()),
            under_dispute: false,
        }];

        dispute(
            &mut accounts,
            Transaction {
                tx_type: TransactionType::Dispute,
                client: 0,
                id: 1,
                amount: None,
                under_dispute: false,
            },
            &mut history,
        )
        .unwrap();

        assert_eq!(accounts.get(0).unwrap().available, 0.to_fixed::<I50F14>());
        assert_eq!(accounts.get(0).unwrap().total, 1.to_fixed::<I50F14>());
        assert_eq!(accounts.get(0).unwrap().held, 1.to_fixed::<I50F14>());
    }
}
