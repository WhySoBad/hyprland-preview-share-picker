fn main() {
    let version =
        match std::process::Command::new("git").arg("describe").arg("--long").arg("--abbrev=7").arg("--tags").output() {
            Ok(output) => {
                let str = String::from_utf8_lossy(&output.stdout);
                let split = str.trim().split('-').collect::<Vec<_>>();
                format!("{}-r{}-{}", &split[0][1..], split[1], split[2])
            }
            Err(err) => {
                println!("cargo::warning=Unable to get git version: {err:#}");
                String::from("unknown")
            }
        };

    let commit = match std::process::Command::new("git").arg("log").arg("--format=[%s]").arg("-n").arg("1").output() {
        Ok(output) => {
            if output.stdout.is_empty() {
                String::from("unknown")
            } else {
                String::from_utf8_lossy(&output.stdout).trim().to_string()
            }
        }
        Err(err) => {
            println!("cargo::warning=Unable to get latest git commit: {err:#}");
            String::from("unknown")
        }
    };

    println!("cargo::rustc-env=GIT_VERSION={version} {commit}");
}
