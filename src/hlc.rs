#[cfg(test)]
use std::cell::RefCell;
use std::cmp::max;
use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

// set a mock thread-local static physical time during testing.
#[cfg(test)]
std::thread_local! {
    static MOCK_PT: RefCell<Option<u64>> = RefCell::new(None);
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Hash, Serialize, Deserialize)]
pub struct Hlc(u64);

/// Hybrid Logical Clock used to uniquely identify each row from a single editor.
impl Hlc {
    #[inline]
    pub fn new(l: u64, c: u16) -> Self {
        let l = l & 0xFFFF_FFFF_FFFF_0000;
        let c = c as u64 & 0x0000_0000_0000_FFFF;
        Hlc(l | c)
    }

    #[inline]
    pub fn to_u64(self) -> u64 {
        self.0
    }

    #[inline]
    pub fn from_u64(i: u64) -> Self {
        Hlc(i)
    }

    #[inline]
    pub fn l(self) -> u64 {
        self.0 & 0xFFFF_FFFF_FFFF_0000
    }

    #[inline]
    pub fn c(self) -> u16 {
        (self.0 & 0xFFFF) as u16
    }

    //// Determines whether a remote HLC is valid. An HLC is valid if
    //// its physical time (pt) is no more than 30s ahead of device pt.
    // #[inline]
    // pub fn is_valid(&self) -> bool {
    //     self.l() <= Self::makept() + 30_000_000
    // }

    /// Creates a new HLC from an existing, local HLC.
    /// If physical time (pt) has changed, l is set to pt and c is set to 0.
    /// If pt has not changed, c is incremented.
    #[inline]
    pub fn next(self) -> Self {
        #[cfg(test)]
        if let Some(pt) = MOCK_PT.with(|f| (*f.borrow()).clone()) {
            return self.next_inner(pt);
        }
        self.next_inner(Self::makept())
    }

    /// Increments the HLC by one
    #[inline]
    pub fn inc(self) -> Self {
        Self(self.0 + 1)
    }

    #[inline]
    fn next_inner(self, pt: u64) -> Self {
        let l = max(self.l(), pt);

        if l == self.l() {
            Hlc(self.0 + 1)
        } else {
            Hlc::new(l, 0)
        }
    }

    fn makept() -> u64 {
        let duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time cannot go backwards");
        let nsec = duration.as_secs() * 1_000_000 + duration.subsec_nanos() as u64;
        nsec & 0xFFFF_FFFF_FFFF_0000
    }

    #[cfg(test)]
    pub fn set_mock_pt(pt: u64) {
        MOCK_PT.with(|f| *f.borrow_mut() = Some(pt))
    }

    #[cfg(test)]
    pub fn unset_mock_pt() {
        MOCK_PT.with(|f| *f.borrow_mut() = None)
    }
}

impl fmt::Debug for Hlc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Hlc")
            .field("l", &self.l())
            .field("c", &self.c())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_send_same_instant() {
        let pt = 1_628_999_999_946_752; // Hlc-friendly time is divisible by 0x1_0000
        Hlc::set_mock_pt(pt);

        let a = Hlc::new(0, 0);
        let b = a.next();
        let c = b.next();

        assert_eq!(b.l(), pt);
        assert_eq!(c.l(), pt);
        assert_eq!(b.c(), 0);
        assert_eq!(c.c(), 1);

        Hlc::unset_mock_pt();
    }

    #[test]
    fn test_send_diff_instant() {
        let pt1 = 1_628_999_999_946_752; // Hlc-friendly time is divisible by 0x1_0000
        let pt2 = 1_629_000_000_012_288; // Hlc-friendly time is divisible by 0x1_0000
        let a = Hlc::new(0, 0);

        Hlc::set_mock_pt(pt1);
        let b = a.next();

        Hlc::set_mock_pt(pt2);
        let c = b.next();

        assert_eq!(b.l(), pt1);
        assert_eq!(b.c(), 0);

        assert_eq!(c.l(), pt2);
        assert_eq!(c.c(), 0);

        Hlc::unset_mock_pt();
    }

    #[test]
    fn test_cast() {
        let i = (i64::MAX as u64) + 1;
        let hlc_old = Hlc::new(i, 56);
        let hlc_i64 = hlc_old.to_u64();
        let hlc_new = Hlc::from_u64(hlc_i64);
        assert_eq!(hlc_new, hlc_old);
    }

    #[test]
    fn test_next_overflow() {
        Hlc::set_mock_pt(1_628_999_999_946_752); // Hlc-friendly time is divisible by 0x1_0000
        let mut hlc1 = Hlc::from_u64(0).next();
        hlc1.0 |= u16::MAX as u64;
        let hlc2 = hlc1.next();

        println!("Hlc1.c = {}, u16::MAX = {}", hlc1.c(), u16::MAX);
        dbg!(&hlc1);
        dbg!(&hlc2);

        assert_eq!(hlc2.to_u64(), hlc1.to_u64() + 1);
        assert_eq!(hlc2.c(), 0);
        Hlc::unset_mock_pt();
    }
}
