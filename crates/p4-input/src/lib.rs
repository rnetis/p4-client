//! Unified input abstraction (PRD §11).
//!
//! All applications consume the same `InputEvent` interface. Touch gestures
//! are recognized here and mapped to mouse semantics (tap=left click,
//! long-press=right click, drag=move, two-finger=scroll).

/// Touch phase within a contact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TouchPhase {
    Start,
    Move,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TouchEvent {
    pub x: u16,
    pub y: u16,
    pub phase: TouchPhase,
    /// Number of contacts (1 = single finger, 2 = two-finger scroll, ...).
    pub fingers: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MouseEvent {
    pub x: u16,
    pub y: u16,
    pub buttons: MouseButtons,
    pub wheel: i8,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MouseButtons {
    pub left: bool,
    pub right: bool,
    pub middle: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyEvent {
    /// HID usage code (e.g. 0x04 = 'a').
    pub code: u8,
    pub pressed: bool,
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonEvent {
    Press(PowerButton),
    Release(PowerButton),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerButton {
    Power,
    Reset,
    Boot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputEvent {
    Touch(TouchEvent),
    Mouse(MouseEvent),
    Keyboard(KeyEvent),
    Button(ButtonEvent),
}

/// Recognized touch gestures (PRD §11 Touch).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gesture {
    Tap,
    DoubleTap,
    LongPress,
    Swipe { dx: i16, dy: i16 },
    Drag { dx: i16, dy: i16 },
    Scroll { dy: i8 },
    None_,
}

/// Tunables for the recognizer. Long press threshold 500 ms (typical).
#[derive(Debug, Clone, Copy)]
pub struct GestureConfig {
    pub long_press_ms: u32,
    pub double_tap_gap_ms: u32,
    pub tap_slop_px: u16,
}

impl Default for GestureConfig {
    fn default() -> Self {
        Self {
            long_press_ms: 500,
            double_tap_gap_ms: 300,
            tap_slop_px: 12,
        }
    }
}

/// Incremental gesture recognizer fed raw touch events plus timestamps (ms).
#[derive(Debug, Default)]
pub struct GestureRecognizer {
    config: GestureConfig,
    down_at: Option<u32>,
    start: Option<(u16, u16)>,
    last: Option<(u16, u16)>,
    moved: bool,
    fingers: u8,
    last_tap: Option<(u32, u16, u16)>,
}

impl GestureRecognizer {
    pub fn new(config: GestureConfig) -> Self {
        Self {
            config,
            ..Default::default()
        }
    }

    pub fn feed(&mut self, ev: &TouchEvent, now_ms: u32) -> Gesture {
        self.fingers = ev.fingers.max(1);
        match ev.phase {
            TouchPhase::Start => {
                self.down_at = Some(now_ms);
                self.start = Some((ev.x, ev.y));
                self.last = Some((ev.x, ev.y));
                self.moved = false;
                Gesture::None_
            }
            TouchPhase::Move => {
                if let (Some((sx, sy)), Some((lx, ly))) = (self.start, self.last) {
                    let dx = ev.x as i16 - lx as i16;
                    let dy = ev.y as i16 - ly as i16;
                    let dist = (ev.x as i32 - sx as i32).abs() + (ev.y as i32 - sy as i32).abs();
                    if dist > self.config.tap_slop_px as i32 {
                        self.moved = true;
                    }
                    self.last = Some((ev.x, ev.y));
                    if self.fingers >= 2 {
                        return Gesture::Scroll { dy: dy.clamp(-127, 127) as i8 };
                    }
                    if self.moved {
                        return Gesture::Drag { dx, dy };
                    }
                }
                Gesture::None_
            }
            TouchPhase::End => {
                let started = self.down_at.take();
                self.down_at = None;
                match (started, self.start) {
                    (Some(t0), Some((sx, sy))) => {
                        let dur = now_ms.saturating_sub(t0);
                        let dist =
                            (ev.x as i32 - sx as i32).abs() + (ev.y as i32 - sy as i32).abs();
                        if self.fingers >= 2 {
                            return Gesture::None_;
                        }
                        if dur >= self.config.long_press_ms && dist <= self.config.tap_slop_px as i32
                        {
                            return Gesture::LongPress;
                        }
                        if dist > self.config.tap_slop_px as i32 {
                            let (dx, dy) = (
                                ev.x as i16 - sx as i16,
                                ev.y as i16 - sy as i16,
                            );
                            return Gesture::Swipe { dx, dy };
                        }
                        // Tap: check double-tap window.
                        if let Some((t1, tx, ty)) = self.last_tap {
                            let gap = now_ms.saturating_sub(t1);
                            let near = (ev.x as i32 - tx as i32).abs()
                                + (ev.y as i32 - ty as i32).abs()
                                <= self.config.tap_slop_px as i32 * 2;
                            self.last_tap = None;
                            if gap <= self.config.double_tap_gap_ms && near {
                                return Gesture::DoubleTap;
                            }
                        }
                        self.last_tap = Some((now_ms, ev.x, ev.y));
                        Gesture::Tap
                    }
                    _ => Gesture::None_,
                }
            }
        }
    }
}

/// Map a gesture to mouse semantics (PRD §11 Mouse table).
pub fn gesture_to_mouse(g: Gesture, x: u16, y: u16) -> Option<MouseEvent> {
    let mk = |buttons: MouseButtons, wheel: i8| MouseEvent { x, y, buttons, wheel };
    match g {
        Gesture::Tap => Some(mk(MouseButtons { left: true, ..Default::default() }, 0)),
        Gesture::LongPress => Some(mk(MouseButtons { right: true, ..Default::default() }, 0)),
        Gesture::Drag { dx, dy } => Some(mk(
            MouseButtons { left: true, ..Default::default() },
            0,
        ))
        .map(|mut e| {
            e.x = (x as i32 + dx as i32).clamp(0, u16::MAX as i32) as u16;
            e.y = (y as i32 + dy as i32).clamp(0, u16::MAX as i32) as u16;
            e
        }),
        Gesture::Scroll { dy } => Some(mk(Default::default(), dy)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tap(rec: &mut GestureRecognizer, t: &mut u32) -> Vec<Gesture> {
        let down = TouchEvent { x: 100, y: 100, phase: TouchPhase::Start, fingers: 1 };
        let up = TouchEvent { x: 100, y: 100, phase: TouchPhase::End, fingers: 1 };
        let g1 = rec.feed(&down, *t);
        *t += 50;
        let g2 = rec.feed(&up, *t);
        vec![g1, g2]
    }

    #[test]
    fn single_tap() {
        let mut rec = GestureRecognizer::default();
        let mut t = 0;
        assert_eq!(tap(&mut rec, &mut t), vec![Gesture::None_, Gesture::Tap]);
    }

    #[test]
    fn double_tap() {
        let mut rec = GestureRecognizer::default();
        let mut t = 0;
        tap(&mut rec, &mut t);
        t += 100; // within 300ms gap
        assert_eq!(tap(&mut rec, &mut t)[1], Gesture::DoubleTap);
    }

    #[test]
    fn two_taps_far_apart_are_single_taps() {
        let mut rec = GestureRecognizer::default();
        let mut t = 0;
        tap(&mut rec, &mut t);
        t += 1000;
        tap(&mut rec, &mut t);
        t += 1000;
        // third tap can't be double (previous pair consumed)
        assert_eq!(tap(&mut rec, &mut t)[1], Gesture::Tap);
    }

    #[test]
    fn long_press() {
        let mut rec = GestureRecognizer::default();
        let down = TouchEvent { x: 10, y: 10, phase: TouchPhase::Start, fingers: 1 };
        let up = TouchEvent { x: 10, y: 10, phase: TouchPhase::End, fingers: 1 };
        assert_eq!(rec.feed(&down, 0), Gesture::None_);
        assert_eq!(rec.feed(&up, 600), Gesture::LongPress);
    }

    #[test]
    fn swipe() {
        let mut rec = GestureRecognizer::default();
        let down = TouchEvent { x: 400, y: 200, phase: TouchPhase::Start, fingers: 1 };
        assert_eq!(rec.feed(&down, 0), Gesture::None_);
        let up = TouchEvent { x: 100, y: 200, phase: TouchPhase::End, fingers: 1 };
        match rec.feed(&up, 150) {
            Gesture::Swipe { dx, dy } => {
                assert!(dx < -100);
                assert_eq!(dy, 0);
            }
            other => panic!("expected swipe, got {other:?}"),
        }
    }

    #[test]
    fn two_finger_move_is_scroll() {
        let mut rec = GestureRecognizer::default();
        let down = TouchEvent { x: 50, y: 50, phase: TouchPhase::Start, fingers: 2 };
        rec.feed(&down, 0);
        let mv = TouchEvent { x: 50, y: 70, phase: TouchPhase::Move, fingers: 2 };
        assert_eq!(rec.feed(&mv, 20), Gesture::Scroll { dy: 20 });
    }

    #[test]
    fn mapping_table() {
        let tap = gesture_to_mouse(Gesture::Tap, 5, 6).unwrap();
        assert!(tap.buttons.left && !tap.buttons.right);
        let lp = gesture_to_mouse(Gesture::LongPress, 5, 6).unwrap();
        assert!(lp.buttons.right && !lp.buttons.left);
        assert!(gesture_to_mouse(Gesture::Swipe { dx: 1, dy: 0 }, 0, 0).is_none());
    }
}
