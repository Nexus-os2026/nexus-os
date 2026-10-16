#[cfg(test)]
mod m {
    #[test]
    fn ordinary_one() {}
    #[test]
    #[ignore]
    fn ignored_one() {}
    mod inner {
        #[test]
        fn witness_like() {}
    }
}
