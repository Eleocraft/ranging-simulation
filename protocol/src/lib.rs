use uwb::MacFrameFactory;

fn main() {
    let poll = MacFrameFactory::new_poll();
    let frame = poll.build();
}
