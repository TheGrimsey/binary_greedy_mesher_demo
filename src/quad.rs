// helper
#[derive(Copy, Clone)]
pub enum Direction {
    Left,
    Right,
    Down,
    Up,
    Back,
    Forward,
}

impl Direction {
    /// normal data is packed in the shader
    pub fn get_normal(&self) -> i32 {
        match self {
            Direction::Left => 0i32,
            Direction::Right => 1i32,
            Direction::Down => 2i32,
            Direction::Up => 3i32,
            Direction::Back => 4i32,
            Direction::Forward => 5i32,
        }
    }

    pub fn get_opposite(self) -> Self {
        match self {
            Direction::Left => Direction::Right,
            Direction::Right => Direction::Left,
            Direction::Down => Direction::Up,
            Direction::Up => Direction::Down,
            Direction::Back => Direction::Forward,
            Direction::Forward => Direction::Back,
        }
    }
}
