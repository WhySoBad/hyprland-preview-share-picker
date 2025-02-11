fn main() {
    let tag = match std::process::Command::new("git").arg("describe").arg("--tags").arg("--abbrev=0").output() {
        Ok(output) => {
            if output.stdout.is_empty() {
                String::from("unknown")
            } else {
                String::from_utf8_lossy(&output.stdout).trim().to_string()
            }
        },
        Err(err) => {
            println!("cargo::warning=Unable to get latest git tag: {err:#}");
            String::from("unknown")
        },
    };

    let commit = match std::process::Command::new("git").arg("log").arg("--format=%H [%s]").arg("-n").arg("1").output() {
        Ok(output) => {
            if output.stdout.is_empty() {
                String::from("unknown")
            } else {
                String::from_utf8_lossy(&output.stdout).trim().to_string()
            }
        },
        Err(err) => {
            println!("cargo::warning=Unable to get latest git commit: {err:#}");
            String::from("unknown")
        },
    };

    println!("cargo::rustc-env=GIT_VERSION={tag} at commit {commit}");
}