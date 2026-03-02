#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScrollState {
    pub offset: usize,
    pub total_lines: usize,
    pub viewport_height: usize,
    pub auto_scroll: bool,
}

impl Default for ScrollState {
    fn default() -> Self {
        Self {
            offset: 0,
            total_lines: 0,
            viewport_height: 0,
            auto_scroll: true,
        }
    }
}

impl ScrollState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn max_scroll(&self) -> usize {
        self.total_lines.saturating_sub(self.viewport_height)
    }

    pub fn effective_offset(&self) -> usize {
        self.offset.min(self.max_scroll())
    }

    pub fn scroll_up(&mut self, lines: usize) {
        self.auto_scroll = false;
        self.offset = self.offset.saturating_sub(lines);
    }

    pub fn scroll_down(&mut self, lines: usize) {
        self.offset = self.offset.saturating_add(lines);
        let max = self.max_scroll();
        if self.offset >= max {
            self.offset = max;
            self.auto_scroll = true;
        }
    }

    pub fn scroll_page_up(&mut self) {
        self.scroll_up(self.viewport_height.saturating_sub(1));
    }

    pub fn scroll_page_down(&mut self) {
        self.scroll_down(self.viewport_height.saturating_sub(1));
    }

    pub fn scroll_to_top(&mut self) {
        self.auto_scroll = false;
        self.offset = 0;
    }

    pub fn scroll_to_bottom(&mut self) {
        self.offset = usize::MAX;
        self.auto_scroll = true;
    }

    pub fn scroll_to_ratio(&mut self, ratio: f32) {
        self.auto_scroll = false;
        let max = self.max_scroll();
        self.offset = ((ratio.clamp(0.0, 1.0) * max as f32) as usize).min(max);
    }

    pub fn update_content(&mut self, total_lines: usize, viewport_height: usize) {
        self.total_lines = total_lines;
        self.viewport_height = viewport_height;

        if self.auto_scroll {
            self.offset = usize::MAX;
        }
    }

    pub fn is_at_bottom(&self) -> bool {
        self.effective_offset() >= self.max_scroll()
    }

    pub fn scroll_ratio(&self) -> f32 {
        let max = self.max_scroll();
        if max == 0 {
            return 0.0;
        }
        self.effective_offset() as f32 / max as f32
    }
}
