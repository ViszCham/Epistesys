pub fn checked_add(a: u32, b: u32) -> Option<u32> {
    a.checked_add(b)
}

#[cfg(test)]
mod tests {
    #[test]
    fn library_runtime_exercises_checked_add() {
        assert_eq!(super::checked_add(2, 2), Some(4));
        assert_eq!(super::checked_add(u32::MAX, 1), None);
    }
}
