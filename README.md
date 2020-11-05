# Payments

## Overview
`payments` is a simple transactions engine, which takes a CSV of transactions and outputs account information derived from those transactions to `stdout`. It can handle `deposits`, `withdrawals`, `disputes`, `resolutions`, and `chargebacks`.

Example transaction input (`input.csv`):
```csv
type,client,tx,amount
deposit,1,1,1.9999
deposit,1,2,0.0001

```

Example account output (`stdout`):
```csv
client,available,held,total,locked
1,2,0,2,false

```

## Quick Start
Either build the project with `cargo build`, then run with `payments input_file.csv`, or run directly with cargo via `cargo run -- input_file`


## Notes

`payments` will try to work through some types of invalid transaction rows:
- It will ignore transactions where the referenced client or transaction id is not valid. 
- It will not complete withdrawals where the withdrawal amount is greater than the available funds.
- Chargebacks and resolves for transactions not under dispute will be ignored
- Disputing a transaction already under dispute will be ignored
- Chargebacks, disputes, and resolves with an amount will ignore the amount but process the transaction otherwise
- Deposits and withdrawals without an amount will be ignored

`payments` will panic on otherwise malformed rows. For example, the amount passed in, the client id, and the transaction id must all be numbers,

To see output from recoverable errors, run the program with a second argument of `--verbose`, ex: `cargo run -- input.csv --verbose`. Note that these errors will also be output to `stdout`.


Transaction values are stored in signed 64bit fixed-point number notation, with 50 bits of integer precision and 14 bits of fractional precision. If future requirements needed integer precision larger than 50 bits (~1 quadrillion), conversion to a 128bit fixed number format would be lossless.
