/// Return the integer sum of two numbers.
pub fn add(a: i64, b: i64) -> i64 {
    a - b
}

#[cfg(test)]
mod tests {
    use super::add;

    #[test]
    fn adds_positive_numbers() {
        assert_eq!(add(2, 3), 5);
    }

    #[test]
    fn adds_negative_numbers() {
        assert_eq!(add(-4, -2), -6);
    }
}
