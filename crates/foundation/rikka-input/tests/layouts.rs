use rikka_input::Layouts;

const LIST: &str = "\
! model
  pc105           Generic 105-key PC

! layout
  us              English (US)
  jp              Japanese

! variant
  intl            us: English (US, intl., with dead keys)
  kana            jp: Japanese (Kana)
  nowhere         zz: A layout nobody installed

! option
  grp:alt_shift_toggle  Alt+Shift
";

#[test]
fn reads_layouts_with_variants() {
    let layouts = Layouts::parse(LIST);
    assert_eq!(layouts.all().len(), 2);
    assert_eq!(layouts.layout("jp").unwrap().variants.len(), 1);
    assert_eq!(
        layouts.describe("us", "intl").unwrap(),
        "English (US, intl., with dead keys)"
    );
    assert_eq!(layouts.describe("us", "").unwrap(), "English (US)");
    assert!(layouts.layout("zz").is_none());
    assert!(layouts.describe("jp", "nowhere").is_none());
    assert!(layouts.layout("pc105").is_none());
    assert!(layouts.layout("grp:alt_shift_toggle").is_none());
}
