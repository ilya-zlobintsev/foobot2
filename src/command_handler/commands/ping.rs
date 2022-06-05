use super::{ExecutableCommand, Result};
use crate::{
    get_version,
    platform::{ExecutionContext, Permissions},
};
use std::time::Instant;
use tokio::fs;

pub struct Command {
    pub startup_time: Instant,
}

#[async_trait]
impl ExecutableCommand for Command {
    async fn execute(
        &self,
        _: Box<dyn ExecutionContext + Send + Sync>,
        _: Vec<String>,
    ) -> Result<Option<String>> {
        let uptime = {
            let duration = self.startup_time.elapsed();

            let minutes = (duration.as_secs() / 60) % 60;
            let hours = (duration.as_secs() / 60) / 60;

            let mut result = String::new();

            if hours != 0 {
                result.push_str(&format!("{}h ", hours));
            };

            if minutes != 0 {
                result.push_str(&format!("{}m ", minutes));
            }

            if result.is_empty() {
                result.push_str(&format!("{}s", duration.as_secs()));
            }

            result
        };

        let smaps = fs::read_to_string("/proc/self/smaps")
            .await
            .expect("Proc FS not found");

        let mut mem_usage = 0; // in KB

        for line in smaps.lines() {
            if line.starts_with("Pss:") || line.starts_with("SwapPss:") {
                let mut split = line.split_whitespace();
                split.next().unwrap();

                let pss = split.next().unwrap();

                mem_usage += pss.parse::<i32>().unwrap();
            }
        }

        Ok(Some(format!(
            "Pong! Version: {}, Uptime {}, RAM usage: {} MiB",
            get_version(),
            uptime,
            mem_usage / 1024,
        )))
    }

    fn get_cooldown(&self) -> u64 {
        5
    }

    fn get_permissions(&self) -> Permissions {
        Permissions::Default
    }
}
