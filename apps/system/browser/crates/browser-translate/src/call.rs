#[derive(Clone, Copy, Debug)]
pub struct Call<'a> {
    pub strings: &'a [&'a str],
    pub to: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Translated {
    pub text: String,
    pub detected: Option<String>,
}
