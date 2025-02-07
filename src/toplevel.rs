use regex::Regex;

#[derive(Clone, Debug)]
pub struct Toplevel {
    pub id: u64,
    pub class: String,
    pub title: String,
}

impl Toplevel {
    pub fn parse(toplevel_list: &str) -> Vec<Toplevel> {
        let regex = Regex::new(r"\[HC>\]|\[HT>\]").expect("should be valid regex");

        let toplevels = toplevel_list
            .split("[HE>]")
            .filter_map(|part| {
                let split = regex.split(part).collect::<Vec<_>>();
                if split.len() != 3 {
                    return None;
                }
                let id = split[0].parse::<u64>().ok()?;
                let class = split[1].to_string();
                let title = split[2].to_string();
                Some(Toplevel { id, class, title })
            })
            .collect::<Vec<_>>();

        return toplevels;
    }
}
