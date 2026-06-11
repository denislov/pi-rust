use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct RenderScheduler {
    requested: bool,
    force: bool,
    min_interval: Duration,
    last_render_at: Option<Instant>,
}

impl RenderScheduler {
    pub fn new(min_interval: Duration) -> Self {
        Self {
            requested: false,
            force: false,
            min_interval,
            last_render_at: None,
        }
    }

    pub fn request(&mut self, force: bool) {
        self.requested = true;
        self.force |= force;
    }

    pub fn should_render_now(&self, now: Instant) -> bool {
        if !self.requested {
            return false;
        }
        if self.force {
            return true;
        }
        self.last_render_at
            .map(|last| now.duration_since(last) >= self.min_interval)
            .unwrap_or(true)
    }

    pub fn mark_rendered(&mut self, now: Instant) -> bool {
        if !self.requested {
            return false;
        }
        self.requested = false;
        self.force = false;
        self.last_render_at = Some(now);
        true
    }
}
