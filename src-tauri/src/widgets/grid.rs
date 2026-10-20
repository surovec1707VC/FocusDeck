//! Grid geometry: widgets occupy rectangles of cells. Collision handling while
//! dragging happens in the UI; the backend only finds free spots for new
//! widgets and validates saved positions.

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

pub const MAX_COLUMNS: i64 = 24;
pub const MAX_HEIGHT: i64 = 24;
pub const MAX_ROW: i64 = 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
    pub x: i64,
    pub y: i64,
    pub w: i64,
    pub h: i64,
}

impl Rect {
    pub fn overlaps(&self, o: &Rect) -> bool {
        self.x < o.x + o.w && o.x < self.x + self.w && self.y < o.y + o.h && o.y < self.y + self.h
    }
}

/// Top-most, then left-most spot where a `w`×`h` widget fits.
pub fn find_free(occupied: &[Rect], w: i64, h: i64, cols: i64) -> (i64, i64) {
    let w = w.clamp(1, cols.max(1));
    for y in 0.. {
        for x in 0..=(cols - w) {
            let r = Rect { x, y, w, h };
            if !occupied.iter().any(|o| o.overlaps(&r)) {
                return (x, y);
            }
        }
    }
    unreachable!("the grid is unbounded downwards")
}

/// Checks a saved position against the grid limits and the widget's minimum size.
pub fn validate(r: &Rect, min: (i64, i64)) -> AppResult<()> {
    let ok = r.x >= 0
        && r.y >= 0
        && r.y <= MAX_ROW
        && r.w >= min.0
        && r.h >= min.1
        && r.h <= MAX_HEIGHT
        && r.x + r.w <= MAX_COLUMNS;
    if ok {
        Ok(())
    } else {
        Err(AppError::Invalid(format!(
            "недопустимое положение виджета: x={}, y={}, ширина={}, высота={}",
            r.x, r.y, r.w, r.h
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn free_spots() {
        assert_eq!(find_free(&[], 4, 2, 12), (0, 0));
        let taken = [Rect { x: 0, y: 0, w: 6, h: 2 }];
        assert_eq!(find_free(&taken, 4, 2, 12), (6, 0));
        assert_eq!(find_free(&taken, 8, 2, 12), (0, 2));
        // Wider than the grid: clamped.
        assert_eq!(find_free(&[], 30, 1, 12), (0, 0));
    }

    #[test]
    fn limits() {
        assert!(validate(&Rect { x: 0, y: 0, w: 3, h: 2 }, (2, 2)).is_ok());
        assert!(validate(&Rect { x: -1, y: 0, w: 3, h: 2 }, (2, 2)).is_err());
        assert!(validate(&Rect { x: 0, y: 0, w: 1, h: 2 }, (2, 2)).is_err());
        assert!(validate(&Rect { x: 20, y: 0, w: 6, h: 2 }, (2, 2)).is_err());
    }
}
