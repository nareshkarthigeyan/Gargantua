//! The soundtrack owns the journey's transport clock. Pause both together;
//! frame drops can never slow the camera relative to the music.
use std::process::{Child, Command, Stdio};
use std::time::Instant;

fn soundtrack_path() -> Option<std::path::PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let parent = exe.parent()?;
    [
        parent.join("soundtrack.m4a"),
        parent.join("../Resources/soundtrack.m4a"),
        std::path::PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/music/soundtrack.m4a"
        )),
    ]
    .into_iter()
    .find(|p| p.is_file())
}

pub struct Music {
    child: Option<Child>,
    running_since: Option<Instant>,
    accumulated: f64,
}
impl Music {
    pub fn new() -> Self {
        Self {
            child: None,
            running_since: None,
            accumulated: 0.0,
        }
    }
    pub fn start(&mut self) {
        self.stop();
        self.accumulated = 0.0;
        if cfg!(target_os = "macos") && soundtrack_path().is_some() {
            match Command::new("/usr/bin/afplay")
                .args(["-v", "0.55"])
                .arg(soundtrack_path().unwrap())
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
            {
                Ok(child) => self.child = Some(child),
                Err(error) => log::warn!("Soundtrack could not start: {error}"),
            }
        }
        self.running_since = Some(Instant::now());
    }
    pub fn elapsed(&self) -> f64 {
        self.accumulated
            + self
                .running_since
                .map_or(0.0, |t| t.elapsed().as_secs_f64())
    }
    pub fn set_paused(&mut self, paused: bool) {
        if paused && self.running_since.is_some() {
            self.accumulated = self.elapsed();
            self.running_since = None;
            self.signal("-STOP");
        } else if !paused && self.running_since.is_none() {
            self.signal("-CONT");
            self.running_since = Some(Instant::now());
        }
    }
    fn signal(&self, signal: &str) {
        if let Some(child) = &self.child {
            let _ = Command::new("/bin/kill")
                .args([signal, &child.id().to_string()])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
    }
    pub fn update(&mut self) {
        if let Some(child) = self.child.as_mut() {
            if let Ok(Some(_)) = child.try_wait() {
                self.child = None;
            }
        }
    }
    fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.running_since = None;
    }
}
impl Drop for Music {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn transport_clock_pauses_and_resumes_without_advancing_while_paused() {
        let mut music = Music::new(); // silent until Play
        assert!(music.child.is_none());
        assert_eq!(music.elapsed(), 0.0);
        music.set_paused(false);
        std::thread::sleep(std::time::Duration::from_millis(5));
        music.set_paused(true);
        let held = music.elapsed();
        std::thread::sleep(std::time::Duration::from_millis(5));
        assert_eq!(music.elapsed(), held);
        music.set_paused(false);
        assert!(music.elapsed() >= held);
    }
}
