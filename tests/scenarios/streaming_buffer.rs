use std::time::{Duration, Instant};

struct StreamingBuffer {
    text: String,
    last_flush: Instant,
    allocated_height: u16,
    last_printed_height: u16,
}

impl StreamingBuffer {
    fn new() -> Self {
        Self {
            text: String::new(),
            last_flush: Instant::now(),
            allocated_height: 0,
            last_printed_height: 0,
        }
    }

    fn push(&mut self, delta: &str) {
        self.text.push_str(delta);
    }

    fn should_flush(&self) -> bool {
        let time_elapsed = self.last_flush.elapsed() >= Duration::from_millis(150);
        let chars_accumulated = self.text.len() >= 32;
        time_elapsed || chars_accumulated
    }

    fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    fn clear(&mut self) {
        self.text.clear();
        self.allocated_height = 0;
        self.last_printed_height = 0;
    }

    fn calculate_height(&self, width: usize) -> u16 {
        if width == 0 {
            return 1;
        }
        let text_len = self.text.chars().count();
        std::cmp::max(1, text_len.div_ceil(width) as u16)
    }
}

#[test]
fn test_empty_buffer_doesnt_flush() {
    let buffer = StreamingBuffer::new();

    assert!(buffer.is_empty());
    assert!(!buffer.should_flush());
}

#[test]
fn test_single_char_accumulation() {
    let mut buffer = StreamingBuffer::new();

    assert!(!buffer.should_flush());

    buffer.push("a");

    assert!(!buffer.is_empty());

    for _ in 0..31 {
        buffer.push("a");
    }

    assert!(buffer.should_flush(), "Should flush after 32 chars");
}

#[test]
fn test_time_based_flush() {
    let mut buffer = StreamingBuffer::new();

    buffer.push("short");

    assert!(!buffer.should_flush(), "Should not flush immediately");

    std::thread::sleep(Duration::from_millis(160));

    assert!(buffer.should_flush(), "Should flush after 150ms");
}

#[test]
fn test_height_calculation() {
    let mut buffer = StreamingBuffer::new();

    let height = buffer.calculate_height(80);
    assert_eq!(height, 1, "Empty buffer should be 1 line");

    buffer.push("This is a test string that is about forty characters");
    let height = buffer.calculate_height(80);
    assert_eq!(height, 1, "Short text should fit in 1 line");

    buffer.push(
        " and here's more text to make it longer and longer and longer and longer and longer",
    );
    let height = buffer.calculate_height(80);
    assert!(height >= 2, "Long text should span multiple lines");
}

#[test]
fn test_clear_resets_state() {
    let mut buffer = StreamingBuffer::new();

    buffer.push("test content");
    buffer.allocated_height = 10;
    buffer.last_printed_height = 5;

    buffer.clear();

    assert!(buffer.is_empty());
    assert_eq!(buffer.allocated_height, 0);
    assert_eq!(buffer.last_printed_height, 0);
}

#[test]
fn test_consecutive_pushes_accumulate() {
    let mut buffer = StreamingBuffer::new();

    buffer.push("Part 1 ");
    buffer.push("Part 2 ");
    buffer.push("Part 3");

    assert_eq!(buffer.text, "Part 1 Part 2 Part 3");
}

#[test]
fn test_very_long_text_height() {
    let mut buffer = StreamingBuffer::new();

    let long_text: String = (0..1000).map(|_| "x").collect();
    buffer.push(&long_text);

    let height = buffer.calculate_height(80);

    assert!(
        height >= 12,
        "1000 chars at 80 width should be at least 12 lines"
    );
}

#[test]
fn test_unicode_text_height() {
    let mut buffer = StreamingBuffer::new();

    buffer.push("你好世界你好世界你好世界你好世界");

    let height = buffer.calculate_height(20);

    assert!(
        height >= 1,
        "Unicode text should calculate height correctly"
    );
}

#[test]
fn test_exact_width_boundary() {
    let mut buffer = StreamingBuffer::new();

    let exact_fit: String = (0..80).map(|_| "x").collect();
    buffer.push(&exact_fit);

    let height = buffer.calculate_height(80);
    assert_eq!(height, 1, "Exactly 80 chars should fit in 1 line");

    buffer.push("x");
    let height = buffer.calculate_height(80);
    assert_eq!(height, 2, "81 chars should wrap to 2 lines");
}

#[test]
fn test_width_zero_edge_case() {
    let mut buffer = StreamingBuffer::new();

    buffer.push("test");

    let height = buffer.calculate_height(0);
    assert_eq!(height, 1, "Zero width should default to 1 line");
}

#[test]
fn test_flush_timing_reset() {
    let mut buffer = StreamingBuffer::new();

    buffer.push("test");
    std::thread::sleep(Duration::from_millis(160));

    assert!(buffer.should_flush());

    buffer.last_flush = Instant::now();

    assert!(
        !buffer.should_flush(),
        "Should not flush immediately after reset"
    );
}
