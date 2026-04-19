use anyhow::Result;

// 基底パーサー
pub trait Parser {
    type Output;
    fn parse(&self, path: &str) -> Result<Self::Output>;
}
