#[test]
fn test_service() {
    #[mrpc::service]
    trait Foo {
        // fn Test2(self, a: i32, b: String);
        fn test2(a: i32, b: String);
        async fn test1(a: i32, b: String);
    }
}
