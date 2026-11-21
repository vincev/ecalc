//! Every `$ ec -e '...'` line in the README, run, against the line after it.

use std::process::Command;

#[test]
fn readme_examples_print_what_they_show() {
    let readme = include_str!("../README.md");
    let lines: Vec<&str> = readme.lines().collect();
    let mut checked = 0;
    let mut failures = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let Some(program) = line
            .strip_prefix("$ ec -e '")
            .and_then(|rest| rest.strip_suffix('\''))
        else {
            continue;
        };
        let want = lines.get(i + 1).copied().unwrap_or("");
        let output = Command::new(env!("CARGO_BIN_EXE_ec"))
            .args(["-e", program])
            .output()
            .expect("run ec");
        let got = String::from_utf8_lossy(&output.stdout);
        let got = got.trim_end();
        if !output.status.success() || got != want {
            failures.push(format!(
                "ec -e '{program}': got {got:?} ({}), the README shows {want:?}",
                String::from_utf8_lossy(&output.stderr).trim_end()
            ));
        }
        checked += 1;
    }
    assert!(checked >= 20, "only {checked} examples found");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
