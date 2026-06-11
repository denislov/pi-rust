use pi_coding_agent::interactive::{Transcript, TranscriptItem};

#[test]
fn transcript_scrolls_within_bounds() {
    let mut transcript = Transcript::new();
    for i in 0..20 {
        transcript.push(TranscriptItem::user(format!("message {i}")));
    }
    transcript.scroll_page_up(5);
    assert_eq!(transcript.scroll_offset(), 5);
    transcript.scroll_page_down(2);
    assert_eq!(transcript.scroll_offset(), 3);
    transcript.scroll_to_bottom();
    assert_eq!(transcript.scroll_offset(), 0);
}
