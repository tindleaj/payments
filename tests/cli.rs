use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::process::Command;

fn cleanup() {
    std::fs::remove_dir_all("./tests/output/").unwrap();
    std::fs::create_dir("./tests/output/").unwrap();
}

#[test]
fn smoke_test() -> Result<(), Box<dyn std::error::Error>> {
    cleanup();

    let expected = std::fs::read_to_string("./tests/expected_output.csv").unwrap();

    let mut cmd = Command::cargo_bin("payments")?;
    cmd.arg("./tests/sample_transactions.csv");

    cmd.assert()
        .success()
        .stdout(predicate::str::similar(expected));

    Ok(())
}
