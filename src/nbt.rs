#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_named_integer_from_root_compound() {
        let input = [10, 0, 0, 3, 0, 1, b'x', 0, 0, 0, 42, 0];
        let root = parse(&input).unwrap();
        assert_eq!(root.get_int("x"), Some(42));
    }

    #[test]
    fn rejects_truncated_integer_payload() {
        let input = [10, 0, 0, 3, 0, 1, b'x', 0, 0];
        assert!(parse(&input).is_err());
    }
}
