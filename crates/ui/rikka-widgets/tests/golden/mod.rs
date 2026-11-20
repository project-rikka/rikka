pub(crate) const DUMPS: &str = r####"=== controls
Column [0.00 0.00 640.00x480.00] content 640.00x322.00
  Row [16.00 16.00 608.00x36.00] content 608.00x36.00
    Button [16.00 16.00 87.00x36.00] content 87.00x29.00
      · quad [16.00 16.00 87.00x36.00]
      Text [32.00 24.00 55.00x21.00] content 55.00x21.00
        · text [32.00 24.00 55.00x21.00] "Primary"
    Button [111.00 16.00 74.00x36.00] content 74.00x29.00
      · quad [111.00 16.00 74.00x36.00]
      Text [127.00 24.00 42.00x21.00] content 42.00x21.00
        · text [127.00 24.00 42.00x21.00] "Ghost"
    Button [193.00 16.00 36.00x36.00] content 30.00x30.00
      · quad [193.00 16.00 36.00x36.00]
      Icon [199.00 22.00 24.00x24.00] content 24.00x24.00
        · icon [199.00 22.00 24.00x24.00]
    Spacer [237.00 34.00 359.00x0.00] content 0.00x0.00
    Spinner [604.00 24.00 20.00x20.00] content 20.00x20.00
      · arc (614.00, 34.00) r 10.00
      · arc (614.00, 34.00) r 10.00
  Row [16.00 64.00 608.00x36.00] content 608.00x36.00
    Checkbox [16.00 64.00 44.00x36.00] content 44.00x12.00
      · quad [24.00 72.00 20.00x20.00]
      · icon [24.00 72.00 20.00x20.00]
    Radio [68.00 64.00 44.00x36.00] content 44.00x12.00
      · quad [76.00 72.00 20.00x20.00]
    Switch [120.00 68.00 46.00x28.00] content 46.00x28.00
      · quad [120.00 68.00 46.00x28.00]
      · quad [141.00 71.00 22.00x22.00]
    Text [174.00 72.00 450.00x21.00] content 61.00x21.00
      · text [174.00 72.00 450.00x21.00] "Switch it"
  Slider [16.00 112.00 608.00x26.00] content 0.00x0.00
    · quad [16.00 122.00 608.00x6.00]
    · quad [16.00 122.00 245.80x6.00]
    · quad [248.80 112.00 26.00x26.00]
  Progress [16.00 150.00 608.00x6.00] content 0.00x0.00
    · quad [16.00 150.00 608.00x6.00]
    · quad [16.00 150.00 364.80x6.00]
  Segmented [16.00 168.00 608.00x42.00] content 192.00x42.00
    · quad [16.00 168.00 608.00x42.00]
    · quad [219.67 171.00 200.67x36.00]
    · text [27.00 178.50 184.67x21.00] "Day" clip [27.00 171.00 184.67x36.00]
    · text [227.67 178.50 184.67x21.00] "Week" clip [227.67 171.00 184.67x36.00]
    · text [428.33 178.50 184.67x21.00] "Month" clip [428.33 171.00 184.67x36.00]
  Dropdown [16.00 222.00 608.00x36.00] content 0.00x0.00
    · quad [16.00 222.00 608.00x36.00]
    · icon [594.00 231.00 18.00x18.00]
    · text [28.00 229.50 558.00x21.00] "First" clip [28.00 222.00 558.00x36.00]
  TextField [16.00 270.00 608.00x36.00] content 24.00x0.00
    · quad [16.00 270.00 608.00x36.00] clip [16.00 270.00 608.00x36.00]
    · text [28.00 278.00 584.00x22.00] "Rikka" clip [28.00 270.00 584.00x36.00]
=== text
Column [0.00 0.00 640.00x480.00] content 1397.00x377.00
  Text [12.00 12.00 616.00x30.00] content 59.00x30.00
    · text [12.00 12.00 616.00x30.00] "A title"
  Text [12.00 50.00 616.00x63.00] content 591.00x63.00
    · text [12.00 50.00 616.00x63.00] "A layout is only as good as the rules it keeps: text wraps inside the room it is given, labels that must stay on one line give way with an ellipsis, and rows that grow share what is left between them."
  Row [12.00 121.00 300.00x21.00] content 1373.00x21.00
    Text [12.00 121.00 271.00x21.00] content 1373.00x21.00
      · text [12.00 121.00 271.00x21.00] "A layout is only as good as the rules…"
    Text [291.00 121.00 21.00x21.00] content 21.00x21.00
      · text [291.00 121.00 21.00x21.00] "tail"
  Column [12.00 150.00 260.00x144.00] content 258.00x144.00
    Text [15.00 150.00 255.00x126.00] content 255.00x126.00
      · text [15.00 150.00 255.00x126.00] "A layout is only as good as the rules it keeps: text wraps inside the room it is given, labels that must stay on one line give way with an ellipsis, and rows that grow share what is left between them."
    Text [119.00 276.00 47.00x18.00] content 47.00x18.00
      · text [119.00 276.00 47.00x18.00] "centred"
  RichText [12.00 302.00 616.00x63.00] content 591.00x63.00
    · text [12.00 302.00 616.00x63.00] "A layout is only as good as the rules it keeps: text wraps inside the room it is given, labels that must stay on one line give way with an ellipsis, and rows that grow share what is left between them."
=== eliding
Column [0.00 0.00 640.00x480.00] content 640.00x84.00
  Row [0.00 0.00 640.00x38.00] content 240.00x38.00
    Container [0.00 0.00 240.00x38.00] content 240.00x38.00
      · quad [0.00 0.00 240.00x38.00]
      Row [10.00 4.00 220.00x30.00] content 220.00x30.00
        Icon [10.00 12.00 15.00x15.00] content 15.00x15.00
          · icon [10.00 12.00 15.00x15.00]
        Text [33.00 11.00 159.00x17.00] content 196.00x17.00
          · text [33.00 11.00 159.00x17.00] "Review screenshot editor…"
        Button [200.00 4.00 30.00x30.00] content 23.00x23.00
          · quad [200.00 4.00 30.00x30.00]
          Icon [208.00 12.00 15.00x15.00] content 15.00x15.00
            · icon [208.00 12.00 15.00x15.00]
  Row [0.00 46.00 640.00x38.00] content 277.00x38.00
    Container [0.00 46.00 277.00x38.00] content 277.00x38.00
      · quad [0.00 46.00 277.00x38.00]
      Row [10.00 50.00 257.00x30.00] content 257.00x30.00
        Icon [10.00 58.00 15.00x15.00] content 15.00x15.00
          · icon [10.00 58.00 15.00x15.00]
        Text [33.00 57.00 196.00x17.00] content 196.00x17.00
          · text [33.00 57.00 196.00x17.00] "Review screenshot editor changes"
        Button [237.00 50.00 30.00x30.00] content 23.00x23.00
          · quad [237.00 50.00 30.00x30.00]
          Icon [245.00 58.00 15.00x15.00] content 15.00x15.00
            · icon [245.00 58.00 15.00x15.00]
=== lists
Row [0.00 0.00 640.00x480.00] content 640.00x480.00
  Collection [0.00 0.00 160.00x480.00] content 152.00x36.00
    · quad [4.00 40.00 152.00x36.00] clip [0.00 0.00 160.00x480.00]
    AnimatedPosition [4.00 4.00 152.00x36.00] content 152.00x36.00 clip [0.00 0.00 160.00x480.00]
      NavItem [4.00 4.00 152.00x36.00] content 0.00x0.00 clip [0.00 0.00 160.00x480.00]
        · text [12.00 11.50 136.00x21.00] "Inbox" clip [12.00 4.00 136.00x36.00]
    AnimatedPosition [4.00 40.00 152.00x36.00] content 152.00x36.00 clip [0.00 0.00 160.00x480.00]
      NavItem [4.00 40.00 152.00x36.00] content 0.00x0.00 clip [0.00 0.00 160.00x480.00]
        · text [12.00 47.50 136.00x21.00] "Drafts" clip [12.00 40.00 136.00x36.00]
    AnimatedPosition [4.00 76.00 152.00x36.00] content 152.00x36.00 clip [0.00 0.00 160.00x480.00]
      NavItem [4.00 76.00 152.00x36.00] content 0.00x0.00 clip [0.00 0.00 160.00x480.00]
        · text [12.00 83.50 136.00x21.00] "Archive" clip [12.00 76.00 136.00x36.00]
  Column [168.00 0.00 472.00x480.00] content 472.00x180.00
    Collection [168.00 0.00 472.00x180.00] content 462.00x36.00
      · quad [168.00 108.00 462.00x36.00] clip [168.00 0.00 472.00x180.00]
      · quad [633.35 2.00 3.30x24.00] clip [168.00 0.00 472.00x180.00]
      Rider [168.00 0.00 462.00x36.00] content 462.00x36.00 clip [168.00 0.00 472.00x180.00]
        ListRow [168.00 0.00 462.00x36.00] content 0.00x0.00 clip [168.00 0.00 472.00x180.00]
          · text [176.00 7.50 446.00x21.00] "row 0" clip [176.00 0.00 446.00x36.00]
      Rider [168.00 36.00 462.00x36.00] content 462.00x36.00 clip [168.00 0.00 472.00x180.00]
        ListRow [168.00 36.00 462.00x36.00] content 0.00x0.00 clip [168.00 0.00 472.00x180.00]
          · text [176.00 43.50 446.00x21.00] "row 1" clip [176.00 36.00 446.00x36.00]
      Rider [168.00 72.00 462.00x36.00] content 462.00x36.00 clip [168.00 0.00 472.00x180.00]
        ListRow [168.00 72.00 462.00x36.00] content 0.00x0.00 clip [168.00 0.00 472.00x180.00]
          · text [176.00 79.50 446.00x21.00] "row 2" clip [176.00 72.00 446.00x36.00]
      Rider [168.00 108.00 462.00x36.00] content 462.00x36.00 clip [168.00 0.00 472.00x180.00]
        ListRow [168.00 108.00 462.00x36.00] content 0.00x0.00 clip [168.00 0.00 472.00x180.00]
          · text [176.00 115.50 446.00x21.00] "row 3" clip [176.00 108.00 446.00x36.00]
      Rider [168.00 144.00 462.00x36.00] content 462.00x36.00 clip [168.00 0.00 472.00x180.00]
        ListRow [168.00 144.00 462.00x36.00] content 0.00x0.00 clip [168.00 0.00 472.00x180.00]
          · text [176.00 151.50 446.00x21.00] "row 4" clip [176.00 144.00 446.00x36.00]
      Rider [168.00 180.00 462.00x36.00] content 462.00x36.00 clip [168.00 0.00 472.00x180.00]
        ListRow [168.00 180.00 462.00x36.00] content 0.00x0.00 clip [168.00 0.00 472.00x180.00]
      Rider [168.00 216.00 462.00x36.00] content 462.00x36.00 clip [168.00 0.00 472.00x180.00]
        ListRow [168.00 216.00 462.00x36.00] content 0.00x0.00 clip [168.00 0.00 472.00x180.00]
      Rider [168.00 252.00 462.00x36.00] content 462.00x36.00 culled
        ListRow [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
      Rider [168.00 288.00 462.00x36.00] content 462.00x36.00 culled
        ListRow [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
      Rider [168.00 324.00 462.00x36.00] content 462.00x36.00 culled
        ListRow [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
      Rider [168.00 360.00 462.00x36.00] content 462.00x36.00 culled
        ListRow [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
=== collections
Row [0.00 0.00 640.00x480.00] content 640.00x480.00
  Collection [0.00 0.00 316.00x480.00] content 306.00x100.00
    · quad [309.35 2.00 3.30x24.00] clip [0.00 0.00 316.00x480.00]
    AnimatedPosition [0.00 0.00 306.00x37.00] content 306.00x37.00 clip [0.00 0.00 316.00x480.00]
      Container [0.00 0.00 306.00x37.00] content 124.00x37.00 clip [0.00 0.00 316.00x480.00]
        Text [8.00 8.00 108.00x21.00] content 108.00x21.00 clip [0.00 0.00 316.00x480.00]
          · text [8.00 8.00 108.00x21.00] "A layout is only" clip [0.00 0.00 316.00x480.00]
    AnimatedPosition [0.00 39.00 306.00x37.00] content 306.00x37.00 clip [0.00 0.00 316.00x480.00]
      Container [0.00 39.00 306.00x37.00] content 232.00x37.00 clip [0.00 0.00 316.00x480.00]
        Text [8.00 47.00 216.00x21.00] content 216.00x21.00 clip [0.00 0.00 316.00x480.00]
          · text [8.00 47.00 216.00x21.00] "A layout is only as good as the" clip [0.00 0.00 316.00x480.00]
    AnimatedPosition [0.00 78.00 306.00x58.00] content 306.00x58.00 clip [0.00 0.00 316.00x480.00]
      Container [0.00 78.00 306.00x58.00] content 306.00x58.00 clip [0.00 0.00 316.00x480.00]
        Text [8.00 86.00 290.00x42.00] content 268.00x42.00 clip [0.00 0.00 316.00x480.00]
          · text [8.00 86.00 290.00x42.00] "A layout is only as good as the rules it keeps: text" clip [0.00 0.00 316.00x480.00]
    AnimatedPosition [0.00 138.00 306.00x58.00] content 306.00x58.00 clip [0.00 0.00 316.00x480.00]
      Container [0.00 138.00 306.00x58.00] content 306.00x58.00 clip [0.00 0.00 316.00x480.00]
        Text [8.00 146.00 290.00x42.00] content 268.00x42.00 clip [0.00 0.00 316.00x480.00]
          · text [8.00 146.00 290.00x42.00] "A layout is only as good as the rules it keeps: text wraps inside the room" clip [0.00 0.00 316.00x480.00]
    AnimatedPosition [0.00 198.00 306.00x79.00] content 306.00x79.00 clip [0.00 0.00 316.00x480.00]
      Container [0.00 198.00 306.00x79.00] content 306.00x79.00 clip [0.00 0.00 316.00x480.00]
        Text [8.00 206.00 290.00x63.00] content 268.00x63.00 clip [0.00 0.00 316.00x480.00]
          · text [8.00 206.00 290.00x63.00] "A layout is only as good as the rules it keeps: text wraps inside the room it is given, labels" clip [0.00 0.00 316.00x480.00]
    AnimatedPosition [0.00 279.00 306.00x79.00] content 306.00x79.00 clip [0.00 0.00 316.00x480.00]
      Container [0.00 279.00 306.00x79.00] content 306.00x79.00 clip [0.00 0.00 316.00x480.00]
        Text [8.00 287.00 290.00x63.00] content 268.00x63.00 clip [0.00 0.00 316.00x480.00]
          · text [8.00 287.00 290.00x63.00] "A layout is only as good as the rules it keeps: text wraps inside the room it is given, labels that must stay on" clip [0.00 0.00 316.00x480.00]
    AnimatedPosition [0.00 360.00 306.00x100.00] content 306.00x100.00 clip [0.00 0.00 316.00x480.00]
      Container [0.00 360.00 306.00x100.00] content 306.00x100.00 clip [0.00 0.00 316.00x480.00]
        Text [8.00 368.00 290.00x84.00] content 275.00x84.00 clip [0.00 0.00 316.00x480.00]
          · text [8.00 368.00 290.00x84.00] "A layout is only as good as the rules it keeps: text wraps inside the room it is given, labels that must stay on one line give way" clip [0.00 0.00 316.00x480.00]
    AnimatedPosition [0.00 462.00 306.00x37.00] content 306.00x37.00 clip [0.00 0.00 316.00x480.00]
      Container [0.00 462.00 306.00x37.00] content 124.00x37.00 clip [0.00 0.00 316.00x480.00]
        Text [8.00 470.00 108.00x21.00] content 108.00x21.00 clip [0.00 0.00 316.00x480.00]
          · text [8.00 470.00 108.00x21.00] "A layout is only" clip [0.00 0.00 316.00x480.00]
    AnimatedPosition [0.00 501.00 306.00x37.00] content 306.00x37.00 clip [0.00 0.00 316.00x480.00]
      Container [0.00 501.00 306.00x37.00] content 232.00x37.00 clip [0.00 0.00 316.00x480.00]
        Text [8.00 509.00 216.00x21.00] content 216.00x21.00 clip [0.00 0.00 316.00x480.00]
          · text [8.00 509.00 216.00x21.00] "A layout is only as good as the" clip [0.00 0.00 316.00x480.00]
    AnimatedPosition [0.00 540.00 306.00x58.00] content 306.00x58.00 clip [0.00 0.00 316.00x480.00]
      Container [0.00 540.00 306.00x58.00] content 306.00x58.00 clip [0.00 0.00 316.00x480.00]
        Text [8.00 548.00 290.00x42.00] content 268.00x42.00 culled
    AnimatedPosition [0.00 600.00 306.00x58.00] content 306.00x58.00 culled
      Container [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
        Text [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
    AnimatedPosition [0.00 660.00 306.00x79.00] content 306.00x79.00 culled
      Container [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
        Text [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
    AnimatedPosition [0.00 741.00 306.00x79.00] content 306.00x79.00 culled
      Container [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
        Text [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
    AnimatedPosition [0.00 822.00 306.00x100.00] content 306.00x100.00 culled
      Container [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
        Text [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
    AnimatedPosition [0.00 924.00 306.00x37.00] content 306.00x37.00 culled
      Container [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
        Text [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
  Collection [324.00 0.00 316.00x480.00] content 102.00x29.00
    · quad [633.35 2.00 3.30x393.93] clip [324.00 0.00 316.00x480.00]
    AnimatedPosition [324.00 0.00 102.00x29.00] content 102.00x29.00 clip [324.00 0.00 316.00x480.00]
      Container [324.00 0.00 102.00x29.00] content 47.00x29.00 clip [324.00 0.00 316.00x480.00]
        Text [328.00 4.00 39.00x21.00] content 39.00x21.00 clip [324.00 0.00 316.00x480.00]
          · text [328.00 4.00 39.00x21.00] "cell 0" clip [324.00 0.00 316.00x480.00]
    AnimatedPosition [426.00 0.00 102.00x29.00] content 102.00x29.00 clip [324.00 0.00 316.00x480.00]
      Container [426.00 0.00 102.00x29.00] content 43.00x29.00 clip [324.00 0.00 316.00x480.00]
        Text [430.00 4.00 35.00x21.00] content 35.00x21.00 clip [324.00 0.00 316.00x480.00]
          · text [430.00 4.00 35.00x21.00] "cell 1" clip [324.00 0.00 316.00x480.00]
    AnimatedPosition [528.00 0.00 102.00x29.00] content 102.00x29.00 clip [324.00 0.00 316.00x480.00]
      Container [528.00 0.00 102.00x29.00] content 46.00x29.00 clip [324.00 0.00 316.00x480.00]
        Text [532.00 4.00 38.00x21.00] content 38.00x21.00 clip [324.00 0.00 316.00x480.00]
          · text [532.00 4.00 38.00x21.00] "cell 2" clip [324.00 0.00 316.00x480.00]
    AnimatedPosition [324.00 29.00 102.00x29.00] content 102.00x29.00 clip [324.00 0.00 316.00x480.00]
      Container [324.00 29.00 102.00x29.00] content 47.00x29.00 clip [324.00 0.00 316.00x480.00]
        Text [328.00 33.00 39.00x21.00] content 39.00x21.00 clip [324.00 0.00 316.00x480.00]
          · text [328.00 33.00 39.00x21.00] "cell 3" clip [324.00 0.00 316.00x480.00]
    AnimatedPosition [426.00 29.00 102.00x29.00] content 102.00x29.00 clip [324.00 0.00 316.00x480.00]
      Container [426.00 29.00 102.00x29.00] content 47.00x29.00 clip [324.00 0.00 316.00x480.00]
        Text [430.00 33.00 39.00x21.00] content 39.00x21.00 clip [324.00 0.00 316.00x480.00]
          · text [430.00 33.00 39.00x21.00] "cell 4" clip [324.00 0.00 316.00x480.00]
    AnimatedPosition [528.00 29.00 102.00x29.00] content 102.00x29.00 clip [324.00 0.00 316.00x480.00]
      Container [528.00 29.00 102.00x29.00] content 46.00x29.00 clip [324.00 0.00 316.00x480.00]
        Text [532.00 33.00 38.00x21.00] content 38.00x21.00 clip [324.00 0.00 316.00x480.00]
          · text [532.00 33.00 38.00x21.00] "cell 5" clip [324.00 0.00 316.00x480.00]
    AnimatedPosition [324.00 58.00 102.00x29.00] content 102.00x29.00 clip [324.00 0.00 316.00x480.00]
      Container [324.00 58.00 102.00x29.00] content 47.00x29.00 clip [324.00 0.00 316.00x480.00]
        Text [328.00 62.00 39.00x21.00] content 39.00x21.00 clip [324.00 0.00 316.00x480.00]
          · text [328.00 62.00 39.00x21.00] "cell 6" clip [324.00 0.00 316.00x480.00]
    AnimatedPosition [426.00 58.00 102.00x29.00] content 102.00x29.00 clip [324.00 0.00 316.00x480.00]
      Container [426.00 58.00 102.00x29.00] content 46.00x29.00 clip [324.00 0.00 316.00x480.00]
        Text [430.00 62.00 38.00x21.00] content 38.00x21.00 clip [324.00 0.00 316.00x480.00]
          · text [430.00 62.00 38.00x21.00] "cell 7" clip [324.00 0.00 316.00x480.00]
    AnimatedPosition [528.00 58.00 102.00x29.00] content 102.00x29.00 clip [324.00 0.00 316.00x480.00]
      Container [528.00 58.00 102.00x29.00] content 47.00x29.00 clip [324.00 0.00 316.00x480.00]
        Text [532.00 62.00 39.00x21.00] content 39.00x21.00 clip [324.00 0.00 316.00x480.00]
          · text [532.00 62.00 39.00x21.00] "cell 8" clip [324.00 0.00 316.00x480.00]
    AnimatedPosition [324.00 87.00 102.00x29.00] content 102.00x29.00 clip [324.00 0.00 316.00x480.00]
      Container [324.00 87.00 102.00x29.00] content 47.00x29.00 clip [324.00 0.00 316.00x480.00]
        Text [328.00 91.00 39.00x21.00] content 39.00x21.00 clip [324.00 0.00 316.00x480.00]
          · text [328.00 91.00 39.00x21.00] "cell 9" clip [324.00 0.00 316.00x480.00]
    AnimatedPosition [426.00 87.00 102.00x29.00] content 102.00x29.00 clip [324.00 0.00 316.00x480.00]
      Container [426.00 87.00 102.00x29.00] content 53.00x29.00 clip [324.00 0.00 316.00x480.00]
        Text [430.00 91.00 45.00x21.00] content 45.00x21.00 clip [324.00 0.00 316.00x480.00]
          · text [430.00 91.00 45.00x21.00] "cell 10" clip [324.00 0.00 316.00x480.00]
    AnimatedPosition [528.00 87.00 102.00x29.00] content 102.00x29.00 clip [324.00 0.00 316.00x480.00]
      Container [528.00 87.00 102.00x29.00] content 50.00x29.00 clip [324.00 0.00 316.00x480.00]
        Text [532.00 91.00 42.00x21.00] content 42.00x21.00 clip [324.00 0.00 316.00x480.00]
          · text [532.00 91.00 42.00x21.00] "cell 11" clip [324.00 0.00 316.00x480.00]
    AnimatedPosition [324.00 116.00 102.00x29.00] content 102.00x29.00 clip [324.00 0.00 316.00x480.00]
      Container [324.00 116.00 102.00x29.00] content 53.00x29.00 clip [324.00 0.00 316.00x480.00]
        Text [328.00 120.00 45.00x21.00] content 45.00x21.00 clip [324.00 0.00 316.00x480.00]
          · text [328.00 120.00 45.00x21.00] "cell 12" clip [324.00 0.00 316.00x480.00]
    AnimatedPosition [426.00 116.00 102.00x29.00] content 102.00x29.00 clip [324.00 0.00 316.00x480.00]
      Container [426.00 116.00 102.00x29.00] content 53.00x29.00 clip [324.00 0.00 316.00x480.00]
        Text [430.00 120.00 45.00x21.00] content 45.00x21.00 clip [324.00 0.00 316.00x480.00]
          · text [430.00 120.00 45.00x21.00] "cell 13" clip [324.00 0.00 316.00x480.00]
    AnimatedPosition [528.00 116.00 102.00x29.00] content 102.00x29.00 clip [324.00 0.00 316.00x480.00]
      Container [528.00 116.00 102.00x29.00] content 53.00x29.00 clip [324.00 0.00 316.00x480.00]
        Text [532.00 120.00 45.00x21.00] content 45.00x21.00 clip [324.00 0.00 316.00x480.00]
          · text [532.00 120.00 45.00x21.00] "cell 14" clip [324.00 0.00 316.00x480.00]
    AnimatedPosition [324.00 145.00 102.00x29.00] content 102.00x29.00 clip [324.00 0.00 316.00x480.00]
      Container [324.00 145.00 102.00x29.00] content 52.00x29.00 clip [324.00 0.00 316.00x480.00]
        Text [328.00 149.00 44.00x21.00] content 44.00x21.00 clip [324.00 0.00 316.00x480.00]
          · text [328.00 149.00 44.00x21.00] "cell 15" clip [324.00 0.00 316.00x480.00]
    AnimatedPosition [426.00 145.00 102.00x29.00] content 102.00x29.00 clip [324.00 0.00 316.00x480.00]
      Container [426.00 145.00 102.00x29.00] content 53.00x29.00 clip [324.00 0.00 316.00x480.00]
        Text [430.00 149.00 45.00x21.00] content 45.00x21.00 clip [324.00 0.00 316.00x480.00]
          · text [430.00 149.00 45.00x21.00] "cell 16" clip [324.00 0.00 316.00x480.00]
    AnimatedPosition [528.00 145.00 102.00x29.00] content 102.00x29.00 clip [324.00 0.00 316.00x480.00]
      Container [528.00 145.00 102.00x29.00] content 52.00x29.00 clip [324.00 0.00 316.00x480.00]
        Text [532.00 149.00 44.00x21.00] content 44.00x21.00 clip [324.00 0.00 316.00x480.00]
          · text [532.00 149.00 44.00x21.00] "cell 17" clip [324.00 0.00 316.00x480.00]
    AnimatedPosition [324.00 174.00 102.00x29.00] content 102.00x29.00 clip [324.00 0.00 316.00x480.00]
      Container [324.00 174.00 102.00x29.00] content 53.00x29.00 clip [324.00 0.00 316.00x480.00]
        Text [328.00 178.00 45.00x21.00] content 45.00x21.00 clip [324.00 0.00 316.00x480.00]
          · text [328.00 178.00 45.00x21.00] "cell 18" clip [324.00 0.00 316.00x480.00]
    AnimatedPosition [426.00 174.00 102.00x29.00] content 102.00x29.00 clip [324.00 0.00 316.00x480.00]
      Container [426.00 174.00 102.00x29.00] content 53.00x29.00 clip [324.00 0.00 316.00x480.00]
        Text [430.00 178.00 45.00x21.00] content 45.00x21.00 clip [324.00 0.00 316.00x480.00]
          · text [430.00 178.00 45.00x21.00] "cell 19" clip [324.00 0.00 316.00x480.00]
    AnimatedPosition [528.00 174.00 102.00x29.00] content 102.00x29.00 clip [324.00 0.00 316.00x480.00]
      Container [528.00 174.00 102.00x29.00] content 56.00x29.00 clip [324.00 0.00 316.00x480.00]
        Text [532.00 178.00 48.00x21.00] content 48.00x21.00 clip [324.00 0.00 316.00x480.00]
          · text [532.00 178.00 48.00x21.00] "cell 20" clip [324.00 0.00 316.00x480.00]
    AnimatedPosition [324.00 203.00 102.00x29.00] content 102.00x29.00 clip [324.00 0.00 316.00x480.00]
      Container [324.00 203.00 102.00x29.00] content 53.00x29.00 clip [324.00 0.00 316.00x480.00]
        Text [328.00 207.00 45.00x21.00] content 45.00x21.00 clip [324.00 0.00 316.00x480.00]
          · text [328.00 207.00 45.00x21.00] "cell 21" clip [324.00 0.00 316.00x480.00]
    AnimatedPosition [426.00 203.00 102.00x29.00] content 102.00x29.00 clip [324.00 0.00 316.00x480.00]
      Container [426.00 203.00 102.00x29.00] content 56.00x29.00 clip [324.00 0.00 316.00x480.00]
        Text [430.00 207.00 48.00x21.00] content 48.00x21.00 clip [324.00 0.00 316.00x480.00]
          · text [430.00 207.00 48.00x21.00] "cell 22" clip [324.00 0.00 316.00x480.00]
    AnimatedPosition [528.00 203.00 102.00x29.00] content 102.00x29.00 clip [324.00 0.00 316.00x480.00]
      Container [528.00 203.00 102.00x29.00] content 56.00x29.00 clip [324.00 0.00 316.00x480.00]
        Text [532.00 207.00 48.00x21.00] content 48.00x21.00 clip [324.00 0.00 316.00x480.00]
          · text [532.00 207.00 48.00x21.00] "cell 23" clip [324.00 0.00 316.00x480.00]
    AnimatedPosition [324.00 232.00 102.00x29.00] content 102.00x29.00 clip [324.00 0.00 316.00x480.00]
      Container [324.00 232.00 102.00x29.00] content 56.00x29.00 clip [324.00 0.00 316.00x480.00]
        Text [328.00 236.00 48.00x21.00] content 48.00x21.00 clip [324.00 0.00 316.00x480.00]
          · text [328.00 236.00 48.00x21.00] "cell 24" clip [324.00 0.00 316.00x480.00]
    AnimatedPosition [426.00 232.00 102.00x29.00] content 102.00x29.00 clip [324.00 0.00 316.00x480.00]
      Container [426.00 232.00 102.00x29.00] content 55.00x29.00 clip [324.00 0.00 316.00x480.00]
        Text [430.00 236.00 47.00x21.00] content 47.00x21.00 clip [324.00 0.00 316.00x480.00]
          · text [430.00 236.00 47.00x21.00] "cell 25" clip [324.00 0.00 316.00x480.00]
    AnimatedPosition [528.00 232.00 102.00x29.00] content 102.00x29.00 clip [324.00 0.00 316.00x480.00]
      Container [528.00 232.00 102.00x29.00] content 56.00x29.00 clip [324.00 0.00 316.00x480.00]
        Text [532.00 236.00 48.00x21.00] content 48.00x21.00 clip [324.00 0.00 316.00x480.00]
          · text [532.00 236.00 48.00x21.00] "cell 26" clip [324.00 0.00 316.00x480.00]
    AnimatedPosition [324.00 261.00 102.00x29.00] content 102.00x29.00 clip [324.00 0.00 316.00x480.00]
      Container [324.00 261.00 102.00x29.00] content 55.00x29.00 clip [324.00 0.00 316.00x480.00]
        Text [328.00 265.00 47.00x21.00] content 47.00x21.00 clip [324.00 0.00 316.00x480.00]
          · text [328.00 265.00 47.00x21.00] "cell 27" clip [324.00 0.00 316.00x480.00]
    AnimatedPosition [426.00 261.00 102.00x29.00] content 102.00x29.00 clip [324.00 0.00 316.00x480.00]
      Container [426.00 261.00 102.00x29.00] content 56.00x29.00 clip [324.00 0.00 316.00x480.00]
        Text [430.00 265.00 48.00x21.00] content 48.00x21.00 clip [324.00 0.00 316.00x480.00]
          · text [430.00 265.00 48.00x21.00] "cell 28" clip [324.00 0.00 316.00x480.00]
    AnimatedPosition [528.00 261.00 102.00x29.00] content 102.00x29.00 clip [324.00 0.00 316.00x480.00]
      Container [528.00 261.00 102.00x29.00] content 56.00x29.00 clip [324.00 0.00 316.00x480.00]
        Text [532.00 265.00 48.00x21.00] content 48.00x21.00 clip [324.00 0.00 316.00x480.00]
          · text [532.00 265.00 48.00x21.00] "cell 29" clip [324.00 0.00 316.00x480.00]
    AnimatedPosition [324.00 290.00 102.00x29.00] content 102.00x29.00 clip [324.00 0.00 316.00x480.00]
      Container [324.00 290.00 102.00x29.00] content 56.00x29.00 clip [324.00 0.00 316.00x480.00]
        Text [328.00 294.00 48.00x21.00] content 48.00x21.00 clip [324.00 0.00 316.00x480.00]
          · text [328.00 294.00 48.00x21.00] "cell 30" clip [324.00 0.00 316.00x480.00]
    AnimatedPosition [426.00 290.00 102.00x29.00] content 102.00x29.00 clip [324.00 0.00 316.00x480.00]
      Container [426.00 290.00 102.00x29.00] content 53.00x29.00 clip [324.00 0.00 316.00x480.00]
        Text [430.00 294.00 45.00x21.00] content 45.00x21.00 clip [324.00 0.00 316.00x480.00]
          · text [430.00 294.00 45.00x21.00] "cell 31" clip [324.00 0.00 316.00x480.00]
    AnimatedPosition [528.00 290.00 102.00x29.00] content 102.00x29.00 clip [324.00 0.00 316.00x480.00]
      Container [528.00 290.00 102.00x29.00] content 56.00x29.00 clip [324.00 0.00 316.00x480.00]
        Text [532.00 294.00 48.00x21.00] content 48.00x21.00 clip [324.00 0.00 316.00x480.00]
          · text [532.00 294.00 48.00x21.00] "cell 32" clip [324.00 0.00 316.00x480.00]
    AnimatedPosition [324.00 319.00 102.00x29.00] content 102.00x29.00 clip [324.00 0.00 316.00x480.00]
      Container [324.00 319.00 102.00x29.00] content 56.00x29.00 clip [324.00 0.00 316.00x480.00]
        Text [328.00 323.00 48.00x21.00] content 48.00x21.00 clip [324.00 0.00 316.00x480.00]
          · text [328.00 323.00 48.00x21.00] "cell 33" clip [324.00 0.00 316.00x480.00]
    AnimatedPosition [426.00 319.00 102.00x29.00] content 102.00x29.00 clip [324.00 0.00 316.00x480.00]
      Container [426.00 319.00 102.00x29.00] content 56.00x29.00 clip [324.00 0.00 316.00x480.00]
        Text [430.00 323.00 48.00x21.00] content 48.00x21.00 clip [324.00 0.00 316.00x480.00]
          · text [430.00 323.00 48.00x21.00] "cell 34" clip [324.00 0.00 316.00x480.00]
    AnimatedPosition [528.00 319.00 102.00x29.00] content 102.00x29.00 clip [324.00 0.00 316.00x480.00]
      Container [528.00 319.00 102.00x29.00] content 55.00x29.00 clip [324.00 0.00 316.00x480.00]
        Text [532.00 323.00 47.00x21.00] content 47.00x21.00 clip [324.00 0.00 316.00x480.00]
          · text [532.00 323.00 47.00x21.00] "cell 35" clip [324.00 0.00 316.00x480.00]
    AnimatedPosition [324.00 348.00 102.00x29.00] content 102.00x29.00 clip [324.00 0.00 316.00x480.00]
      Container [324.00 348.00 102.00x29.00] content 56.00x29.00 clip [324.00 0.00 316.00x480.00]
        Text [328.00 352.00 48.00x21.00] content 48.00x21.00 clip [324.00 0.00 316.00x480.00]
          · text [328.00 352.00 48.00x21.00] "cell 36" clip [324.00 0.00 316.00x480.00]
    AnimatedPosition [426.00 348.00 102.00x29.00] content 102.00x29.00 clip [324.00 0.00 316.00x480.00]
      Container [426.00 348.00 102.00x29.00] content 55.00x29.00 clip [324.00 0.00 316.00x480.00]
        Text [430.00 352.00 47.00x21.00] content 47.00x21.00 clip [324.00 0.00 316.00x480.00]
          · text [430.00 352.00 47.00x21.00] "cell 37" clip [324.00 0.00 316.00x480.00]
    AnimatedPosition [528.00 348.00 102.00x29.00] content 102.00x29.00 clip [324.00 0.00 316.00x480.00]
      Container [528.00 348.00 102.00x29.00] content 56.00x29.00 clip [324.00 0.00 316.00x480.00]
        Text [532.00 352.00 48.00x21.00] content 48.00x21.00 clip [324.00 0.00 316.00x480.00]
          · text [532.00 352.00 48.00x21.00] "cell 38" clip [324.00 0.00 316.00x480.00]
    AnimatedPosition [324.00 377.00 102.00x29.00] content 102.00x29.00 clip [324.00 0.00 316.00x480.00]
      Container [324.00 377.00 102.00x29.00] content 56.00x29.00 clip [324.00 0.00 316.00x480.00]
        Text [328.00 381.00 48.00x21.00] content 48.00x21.00 clip [324.00 0.00 316.00x480.00]
          · text [328.00 381.00 48.00x21.00] "cell 39" clip [324.00 0.00 316.00x480.00]
    AnimatedPosition [426.00 377.00 102.00x29.00] content 102.00x29.00 clip [324.00 0.00 316.00x480.00]
      Container [426.00 377.00 102.00x29.00] content 56.00x29.00 clip [324.00 0.00 316.00x480.00]
        Text [430.00 381.00 48.00x21.00] content 48.00x21.00 clip [324.00 0.00 316.00x480.00]
          · text [430.00 381.00 48.00x21.00] "cell 40" clip [324.00 0.00 316.00x480.00]
    AnimatedPosition [528.00 377.00 102.00x29.00] content 102.00x29.00 clip [324.00 0.00 316.00x480.00]
      Container [528.00 377.00 102.00x29.00] content 53.00x29.00 clip [324.00 0.00 316.00x480.00]
        Text [532.00 381.00 45.00x21.00] content 45.00x21.00 clip [324.00 0.00 316.00x480.00]
          · text [532.00 381.00 45.00x21.00] "cell 41" clip [324.00 0.00 316.00x480.00]
    AnimatedPosition [324.00 406.00 102.00x29.00] content 102.00x29.00 clip [324.00 0.00 316.00x480.00]
      Container [324.00 406.00 102.00x29.00] content 56.00x29.00 clip [324.00 0.00 316.00x480.00]
        Text [328.00 410.00 48.00x21.00] content 48.00x21.00 clip [324.00 0.00 316.00x480.00]
          · text [328.00 410.00 48.00x21.00] "cell 42" clip [324.00 0.00 316.00x480.00]
    AnimatedPosition [426.00 406.00 102.00x29.00] content 102.00x29.00 clip [324.00 0.00 316.00x480.00]
      Container [426.00 406.00 102.00x29.00] content 56.00x29.00 clip [324.00 0.00 316.00x480.00]
        Text [430.00 410.00 48.00x21.00] content 48.00x21.00 clip [324.00 0.00 316.00x480.00]
          · text [430.00 410.00 48.00x21.00] "cell 43" clip [324.00 0.00 316.00x480.00]
    AnimatedPosition [528.00 406.00 102.00x29.00] content 102.00x29.00 clip [324.00 0.00 316.00x480.00]
      Container [528.00 406.00 102.00x29.00] content 57.00x29.00 clip [324.00 0.00 316.00x480.00]
        Text [532.00 410.00 49.00x21.00] content 49.00x21.00 clip [324.00 0.00 316.00x480.00]
          · text [532.00 410.00 49.00x21.00] "cell 44" clip [324.00 0.00 316.00x480.00]
    AnimatedPosition [324.00 435.00 102.00x29.00] content 102.00x29.00 clip [324.00 0.00 316.00x480.00]
      Container [324.00 435.00 102.00x29.00] content 56.00x29.00 clip [324.00 0.00 316.00x480.00]
        Text [328.00 439.00 48.00x21.00] content 48.00x21.00 clip [324.00 0.00 316.00x480.00]
          · text [328.00 439.00 48.00x21.00] "cell 45" clip [324.00 0.00 316.00x480.00]
    AnimatedPosition [426.00 435.00 102.00x29.00] content 102.00x29.00 clip [324.00 0.00 316.00x480.00]
      Container [426.00 435.00 102.00x29.00] content 56.00x29.00 clip [324.00 0.00 316.00x480.00]
        Text [430.00 439.00 48.00x21.00] content 48.00x21.00 clip [324.00 0.00 316.00x480.00]
          · text [430.00 439.00 48.00x21.00] "cell 46" clip [324.00 0.00 316.00x480.00]
    AnimatedPosition [528.00 435.00 102.00x29.00] content 102.00x29.00 clip [324.00 0.00 316.00x480.00]
      Container [528.00 435.00 102.00x29.00] content 55.00x29.00 clip [324.00 0.00 316.00x480.00]
        Text [532.00 439.00 47.00x21.00] content 47.00x21.00 clip [324.00 0.00 316.00x480.00]
          · text [532.00 439.00 47.00x21.00] "cell 47" clip [324.00 0.00 316.00x480.00]
    AnimatedPosition [324.00 464.00 102.00x29.00] content 102.00x29.00 clip [324.00 0.00 316.00x480.00]
      Container [324.00 464.00 102.00x29.00] content 56.00x29.00 clip [324.00 0.00 316.00x480.00]
        Text [328.00 468.00 48.00x21.00] content 48.00x21.00 clip [324.00 0.00 316.00x480.00]
          · text [328.00 468.00 48.00x21.00] "cell 48" clip [324.00 0.00 316.00x480.00]
    AnimatedPosition [426.00 464.00 102.00x29.00] content 102.00x29.00 clip [324.00 0.00 316.00x480.00]
      Container [426.00 464.00 102.00x29.00] content 56.00x29.00 clip [324.00 0.00 316.00x480.00]
        Text [430.00 468.00 48.00x21.00] content 48.00x21.00 clip [324.00 0.00 316.00x480.00]
          · text [430.00 468.00 48.00x21.00] "cell 49" clip [324.00 0.00 316.00x480.00]
    AnimatedPosition [528.00 464.00 102.00x29.00] content 102.00x29.00 clip [324.00 0.00 316.00x480.00]
      Container [528.00 464.00 102.00x29.00] content 56.00x29.00 clip [324.00 0.00 316.00x480.00]
        Text [532.00 468.00 48.00x21.00] content 48.00x21.00 clip [324.00 0.00 316.00x480.00]
          · text [532.00 468.00 48.00x21.00] "cell 50" clip [324.00 0.00 316.00x480.00]
    AnimatedPosition [324.00 493.00 102.00x29.00] content 102.00x29.00 clip [324.00 0.00 316.00x480.00]
      Container [324.00 493.00 102.00x29.00] content 52.00x29.00 clip [324.00 0.00 316.00x480.00]
        Text [328.00 497.00 44.00x21.00] content 44.00x21.00 clip [324.00 0.00 316.00x480.00]
          · text [328.00 497.00 44.00x21.00] "cell 51" clip [324.00 0.00 316.00x480.00]
    AnimatedPosition [426.00 493.00 102.00x29.00] content 102.00x29.00 clip [324.00 0.00 316.00x480.00]
      Container [426.00 493.00 102.00x29.00] content 55.00x29.00 clip [324.00 0.00 316.00x480.00]
        Text [430.00 497.00 47.00x21.00] content 47.00x21.00 clip [324.00 0.00 316.00x480.00]
          · text [430.00 497.00 47.00x21.00] "cell 52" clip [324.00 0.00 316.00x480.00]
    AnimatedPosition [528.00 493.00 102.00x29.00] content 102.00x29.00 clip [324.00 0.00 316.00x480.00]
      Container [528.00 493.00 102.00x29.00] content 55.00x29.00 clip [324.00 0.00 316.00x480.00]
        Text [532.00 497.00 47.00x21.00] content 47.00x21.00 clip [324.00 0.00 316.00x480.00]
          · text [532.00 497.00 47.00x21.00] "cell 53" clip [324.00 0.00 316.00x480.00]
    AnimatedPosition [324.00 522.00 102.00x29.00] content 102.00x29.00 clip [324.00 0.00 316.00x480.00]
      Container [324.00 522.00 102.00x29.00] content 56.00x29.00 clip [324.00 0.00 316.00x480.00]
        Text [328.00 526.00 48.00x21.00] content 48.00x21.00 clip [324.00 0.00 316.00x480.00]
          · text [328.00 526.00 48.00x21.00] "cell 54" clip [324.00 0.00 316.00x480.00]
    AnimatedPosition [426.00 522.00 102.00x29.00] content 102.00x29.00 clip [324.00 0.00 316.00x480.00]
      Container [426.00 522.00 102.00x29.00] content 55.00x29.00 clip [324.00 0.00 316.00x480.00]
        Text [430.00 526.00 47.00x21.00] content 47.00x21.00 clip [324.00 0.00 316.00x480.00]
          · text [430.00 526.00 47.00x21.00] "cell 55" clip [324.00 0.00 316.00x480.00]
    AnimatedPosition [528.00 522.00 102.00x29.00] content 102.00x29.00 clip [324.00 0.00 316.00x480.00]
      Container [528.00 522.00 102.00x29.00] content 55.00x29.00 clip [324.00 0.00 316.00x480.00]
        Text [532.00 526.00 47.00x21.00] content 47.00x21.00 clip [324.00 0.00 316.00x480.00]
          · text [532.00 526.00 47.00x21.00] "cell 56" clip [324.00 0.00 316.00x480.00]
    AnimatedPosition [324.00 551.00 102.00x29.00] content 102.00x29.00 culled
      Container [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
        Text [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
    AnimatedPosition [426.00 551.00 102.00x29.00] content 102.00x29.00 culled
      Container [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
        Text [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
    AnimatedPosition [528.00 551.00 102.00x29.00] content 102.00x29.00 culled
      Container [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
        Text [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
=== tiles
Column [0.00 0.00 640.00x480.00] content 640.00x480.00
  Collection [0.00 0.00 640.00x440.00] content 120.00x90.00
    · quad [633.35 2.00 3.30x97.88] clip [0.00 0.00 640.00x440.00]
    AnimatedPosition [0.00 0.00 120.00x90.00] content 120.00x90.00 clip [0.00 0.00 640.00x440.00]
      Container [0.00 0.00 120.00x90.00] content 35.00x90.00 clip [0.00 0.00 640.00x440.00]
        Text [0.00 0.00 35.00x90.00] content 35.00x21.00 clip [0.00 0.00 640.00x440.00]
          · text [0.00 0.00 35.00x90.00] "tile 0" clip [0.00 0.00 640.00x440.00]
    AnimatedPosition [128.00 0.00 120.00x90.00] content 120.00x90.00 clip [0.00 0.00 640.00x440.00]
      Container [128.00 0.00 120.00x90.00] content 32.00x90.00 clip [0.00 0.00 640.00x440.00]
        Text [128.00 0.00 32.00x90.00] content 32.00x21.00 clip [0.00 0.00 640.00x440.00]
          · text [128.00 0.00 32.00x90.00] "tile 1" clip [0.00 0.00 640.00x440.00]
    AnimatedPosition [256.00 0.00 120.00x90.00] content 120.00x90.00 clip [0.00 0.00 640.00x440.00]
      Container [256.00 0.00 120.00x90.00] content 35.00x90.00 clip [0.00 0.00 640.00x440.00]
        Text [256.00 0.00 35.00x90.00] content 35.00x21.00 clip [0.00 0.00 640.00x440.00]
          · text [256.00 0.00 35.00x90.00] "tile 2" clip [0.00 0.00 640.00x440.00]
    AnimatedPosition [384.00 0.00 120.00x90.00] content 120.00x90.00 clip [0.00 0.00 640.00x440.00]
      Container [384.00 0.00 120.00x90.00] content 35.00x90.00 clip [0.00 0.00 640.00x440.00]
        Text [384.00 0.00 35.00x90.00] content 35.00x21.00 clip [0.00 0.00 640.00x440.00]
          · text [384.00 0.00 35.00x90.00] "tile 3" clip [0.00 0.00 640.00x440.00]
    AnimatedPosition [0.00 98.00 120.00x90.00] content 120.00x90.00 clip [0.00 0.00 640.00x440.00]
      Container [0.00 98.00 120.00x90.00] content 35.00x90.00 clip [0.00 0.00 640.00x440.00]
        Text [0.00 98.00 35.00x90.00] content 35.00x21.00 clip [0.00 0.00 640.00x440.00]
          · text [0.00 98.00 35.00x90.00] "tile 4" clip [0.00 0.00 640.00x440.00]
    AnimatedPosition [128.00 98.00 120.00x90.00] content 120.00x90.00 clip [0.00 0.00 640.00x440.00]
      Container [128.00 98.00 120.00x90.00] content 35.00x90.00 clip [0.00 0.00 640.00x440.00]
        Text [128.00 98.00 35.00x90.00] content 35.00x21.00 clip [0.00 0.00 640.00x440.00]
          · text [128.00 98.00 35.00x90.00] "tile 5" clip [0.00 0.00 640.00x440.00]
    AnimatedPosition [256.00 98.00 120.00x90.00] content 120.00x90.00 clip [0.00 0.00 640.00x440.00]
      Container [256.00 98.00 120.00x90.00] content 35.00x90.00 clip [0.00 0.00 640.00x440.00]
        Text [256.00 98.00 35.00x90.00] content 35.00x21.00 clip [0.00 0.00 640.00x440.00]
          · text [256.00 98.00 35.00x90.00] "tile 6" clip [0.00 0.00 640.00x440.00]
    AnimatedPosition [384.00 98.00 120.00x90.00] content 120.00x90.00 clip [0.00 0.00 640.00x440.00]
      Container [384.00 98.00 120.00x90.00] content 34.00x90.00 clip [0.00 0.00 640.00x440.00]
        Text [384.00 98.00 34.00x90.00] content 34.00x21.00 clip [0.00 0.00 640.00x440.00]
          · text [384.00 98.00 34.00x90.00] "tile 7" clip [0.00 0.00 640.00x440.00]
    AnimatedPosition [0.00 196.00 120.00x90.00] content 120.00x90.00 clip [0.00 0.00 640.00x440.00]
      Container [0.00 196.00 120.00x90.00] content 35.00x90.00 clip [0.00 0.00 640.00x440.00]
        Text [0.00 196.00 35.00x90.00] content 35.00x21.00 clip [0.00 0.00 640.00x440.00]
          · text [0.00 196.00 35.00x90.00] "tile 8" clip [0.00 0.00 640.00x440.00]
    AnimatedPosition [128.00 196.00 120.00x90.00] content 120.00x90.00 clip [0.00 0.00 640.00x440.00]
      Container [128.00 196.00 120.00x90.00] content 35.00x90.00 clip [0.00 0.00 640.00x440.00]
        Text [128.00 196.00 35.00x90.00] content 35.00x21.00 clip [0.00 0.00 640.00x440.00]
          · text [128.00 196.00 35.00x90.00] "tile 9" clip [0.00 0.00 640.00x440.00]
    AnimatedPosition [256.00 196.00 120.00x90.00] content 120.00x90.00 clip [0.00 0.00 640.00x440.00]
      Container [256.00 196.00 120.00x90.00] content 41.00x90.00 clip [0.00 0.00 640.00x440.00]
        Text [256.00 196.00 41.00x90.00] content 41.00x21.00 clip [0.00 0.00 640.00x440.00]
          · text [256.00 196.00 41.00x90.00] "tile 10" clip [0.00 0.00 640.00x440.00]
    AnimatedPosition [384.00 196.00 120.00x90.00] content 120.00x90.00 clip [0.00 0.00 640.00x440.00]
      Container [384.00 196.00 120.00x90.00] content 38.00x90.00 clip [0.00 0.00 640.00x440.00]
        Text [384.00 196.00 38.00x90.00] content 38.00x21.00 clip [0.00 0.00 640.00x440.00]
          · text [384.00 196.00 38.00x90.00] "tile 11" clip [0.00 0.00 640.00x440.00]
    AnimatedPosition [0.00 294.00 120.00x90.00] content 120.00x90.00 clip [0.00 0.00 640.00x440.00]
      Container [0.00 294.00 120.00x90.00] content 41.00x90.00 clip [0.00 0.00 640.00x440.00]
        Text [0.00 294.00 41.00x90.00] content 41.00x21.00 clip [0.00 0.00 640.00x440.00]
          · text [0.00 294.00 41.00x90.00] "tile 12" clip [0.00 0.00 640.00x440.00]
    AnimatedPosition [128.00 294.00 120.00x90.00] content 120.00x90.00 clip [0.00 0.00 640.00x440.00]
      Container [128.00 294.00 120.00x90.00] content 41.00x90.00 clip [0.00 0.00 640.00x440.00]
        Text [128.00 294.00 41.00x90.00] content 41.00x21.00 clip [0.00 0.00 640.00x440.00]
          · text [128.00 294.00 41.00x90.00] "tile 13" clip [0.00 0.00 640.00x440.00]
    AnimatedPosition [256.00 294.00 120.00x90.00] content 120.00x90.00 clip [0.00 0.00 640.00x440.00]
      Container [256.00 294.00 120.00x90.00] content 41.00x90.00 clip [0.00 0.00 640.00x440.00]
        Text [256.00 294.00 41.00x90.00] content 41.00x21.00 clip [0.00 0.00 640.00x440.00]
          · text [256.00 294.00 41.00x90.00] "tile 14" clip [0.00 0.00 640.00x440.00]
    AnimatedPosition [384.00 294.00 120.00x90.00] content 120.00x90.00 clip [0.00 0.00 640.00x440.00]
      Container [384.00 294.00 120.00x90.00] content 41.00x90.00 clip [0.00 0.00 640.00x440.00]
        Text [384.00 294.00 41.00x90.00] content 41.00x21.00 clip [0.00 0.00 640.00x440.00]
          · text [384.00 294.00 41.00x90.00] "tile 15" clip [0.00 0.00 640.00x440.00]
    AnimatedPosition [0.00 392.00 120.00x90.00] content 120.00x90.00 clip [0.00 0.00 640.00x440.00]
      Container [0.00 392.00 120.00x90.00] content 41.00x90.00 clip [0.00 0.00 640.00x440.00]
        Text [0.00 392.00 41.00x90.00] content 41.00x21.00 clip [0.00 0.00 640.00x440.00]
          · text [0.00 392.00 41.00x90.00] "tile 16" clip [0.00 0.00 640.00x440.00]
    AnimatedPosition [128.00 392.00 120.00x90.00] content 120.00x90.00 clip [0.00 0.00 640.00x440.00]
      Container [128.00 392.00 120.00x90.00] content 40.00x90.00 clip [0.00 0.00 640.00x440.00]
        Text [128.00 392.00 40.00x90.00] content 40.00x21.00 clip [0.00 0.00 640.00x440.00]
          · text [128.00 392.00 40.00x90.00] "tile 17" clip [0.00 0.00 640.00x440.00]
    AnimatedPosition [256.00 392.00 120.00x90.00] content 120.00x90.00 clip [0.00 0.00 640.00x440.00]
      Container [256.00 392.00 120.00x90.00] content 41.00x90.00 clip [0.00 0.00 640.00x440.00]
        Text [256.00 392.00 41.00x90.00] content 41.00x21.00 clip [0.00 0.00 640.00x440.00]
          · text [256.00 392.00 41.00x90.00] "tile 18" clip [0.00 0.00 640.00x440.00]
    AnimatedPosition [384.00 392.00 120.00x90.00] content 120.00x90.00 clip [0.00 0.00 640.00x440.00]
      Container [384.00 392.00 120.00x90.00] content 41.00x90.00 clip [0.00 0.00 640.00x440.00]
        Text [384.00 392.00 41.00x90.00] content 41.00x21.00 clip [0.00 0.00 640.00x440.00]
          · text [384.00 392.00 41.00x90.00] "tile 19" clip [0.00 0.00 640.00x440.00]
    AnimatedPosition [0.00 490.00 120.00x90.00] content 120.00x90.00 clip [0.00 0.00 640.00x440.00]
      Container [0.00 490.00 120.00x90.00] content 44.00x90.00 clip [0.00 0.00 640.00x440.00]
        Text [0.00 490.00 44.00x90.00] content 44.00x21.00 clip [0.00 0.00 640.00x440.00]
          · text [0.00 490.00 44.00x90.00] "tile 20" clip [0.00 0.00 640.00x440.00]
    AnimatedPosition [128.00 490.00 120.00x90.00] content 120.00x90.00 clip [0.00 0.00 640.00x440.00]
      Container [128.00 490.00 120.00x90.00] content 41.00x90.00 clip [0.00 0.00 640.00x440.00]
        Text [128.00 490.00 41.00x90.00] content 41.00x21.00 clip [0.00 0.00 640.00x440.00]
          · text [128.00 490.00 41.00x90.00] "tile 21" clip [0.00 0.00 640.00x440.00]
    AnimatedPosition [256.00 490.00 120.00x90.00] content 120.00x90.00 clip [0.00 0.00 640.00x440.00]
      Container [256.00 490.00 120.00x90.00] content 44.00x90.00 clip [0.00 0.00 640.00x440.00]
        Text [256.00 490.00 44.00x90.00] content 44.00x21.00 clip [0.00 0.00 640.00x440.00]
          · text [256.00 490.00 44.00x90.00] "tile 22" clip [0.00 0.00 640.00x440.00]
    AnimatedPosition [384.00 490.00 120.00x90.00] content 120.00x90.00 clip [0.00 0.00 640.00x440.00]
      Container [384.00 490.00 120.00x90.00] content 44.00x90.00 clip [0.00 0.00 640.00x440.00]
        Text [384.00 490.00 44.00x90.00] content 44.00x21.00 clip [0.00 0.00 640.00x440.00]
          · text [384.00 490.00 44.00x90.00] "tile 23" clip [0.00 0.00 640.00x440.00]
    AnimatedPosition [0.00 588.00 120.00x90.00] content 120.00x90.00 culled
      Container [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
        Text [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
    AnimatedPosition [128.00 588.00 120.00x90.00] content 120.00x90.00 culled
      Container [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
        Text [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
    AnimatedPosition [256.00 588.00 120.00x90.00] content 120.00x90.00 culled
      Container [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
        Text [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
    AnimatedPosition [384.00 588.00 120.00x90.00] content 120.00x90.00 culled
      Container [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
        Text [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
    AnimatedPosition [0.00 686.00 120.00x90.00] content 120.00x90.00 culled
      Container [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
        Text [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
    AnimatedPosition [128.00 686.00 120.00x90.00] content 120.00x90.00 culled
      Container [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
        Text [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
    AnimatedPosition [256.00 686.00 120.00x90.00] content 120.00x90.00 culled
      Container [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
        Text [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
    AnimatedPosition [384.00 686.00 120.00x90.00] content 120.00x90.00 culled
      Container [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
        Text [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
    AnimatedPosition [0.00 784.00 120.00x90.00] content 120.00x90.00 culled
      Container [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
        Text [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
    AnimatedPosition [128.00 784.00 120.00x90.00] content 120.00x90.00 culled
      Container [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
        Text [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
    AnimatedPosition [256.00 784.00 120.00x90.00] content 120.00x90.00 culled
      Container [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
        Text [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
    AnimatedPosition [384.00 784.00 120.00x90.00] content 120.00x90.00 culled
      Container [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
        Text [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
  Collection [0.00 440.00 640.00x40.00] content 80.00x30.00
    · quad [2.00 473.35 169.60x3.30] clip [0.00 440.00 640.00x40.00]
    AnimatedPosition [0.00 440.00 80.00x30.00] content 80.00x30.00 clip [0.00 440.00 640.00x40.00]
      Container [0.00 440.00 80.00x30.00] content 52.00x30.00 clip [0.00 440.00 640.00x40.00]
        Text [4.00 444.00 44.00x22.00] content 44.00x21.00 clip [0.00 440.00 640.00x40.00]
          · text [4.00 444.00 44.00x22.00] "chip 0" clip [0.00 440.00 640.00x40.00]
    AnimatedPosition [80.00 440.00 80.00x30.00] content 80.00x30.00 clip [0.00 440.00 640.00x40.00]
      Container [80.00 440.00 80.00x30.00] content 49.00x30.00 clip [0.00 440.00 640.00x40.00]
        Text [84.00 444.00 41.00x22.00] content 41.00x21.00 clip [0.00 440.00 640.00x40.00]
          · text [84.00 444.00 41.00x22.00] "chip 1" clip [0.00 440.00 640.00x40.00]
    AnimatedPosition [160.00 440.00 80.00x30.00] content 80.00x30.00 clip [0.00 440.00 640.00x40.00]
      Container [160.00 440.00 80.00x30.00] content 52.00x30.00 clip [0.00 440.00 640.00x40.00]
        Text [164.00 444.00 44.00x22.00] content 44.00x21.00 clip [0.00 440.00 640.00x40.00]
          · text [164.00 444.00 44.00x22.00] "chip 2" clip [0.00 440.00 640.00x40.00]
    AnimatedPosition [240.00 440.00 80.00x30.00] content 80.00x30.00 clip [0.00 440.00 640.00x40.00]
      Container [240.00 440.00 80.00x30.00] content 52.00x30.00 clip [0.00 440.00 640.00x40.00]
        Text [244.00 444.00 44.00x22.00] content 44.00x21.00 clip [0.00 440.00 640.00x40.00]
          · text [244.00 444.00 44.00x22.00] "chip 3" clip [0.00 440.00 640.00x40.00]
    AnimatedPosition [320.00 440.00 80.00x30.00] content 80.00x30.00 clip [0.00 440.00 640.00x40.00]
      Container [320.00 440.00 80.00x30.00] content 53.00x30.00 clip [0.00 440.00 640.00x40.00]
        Text [324.00 444.00 45.00x22.00] content 45.00x21.00 clip [0.00 440.00 640.00x40.00]
          · text [324.00 444.00 45.00x22.00] "chip 4" clip [0.00 440.00 640.00x40.00]
    AnimatedPosition [400.00 440.00 80.00x30.00] content 80.00x30.00 clip [0.00 440.00 640.00x40.00]
      Container [400.00 440.00 80.00x30.00] content 52.00x30.00 clip [0.00 440.00 640.00x40.00]
        Text [404.00 444.00 44.00x22.00] content 44.00x21.00 clip [0.00 440.00 640.00x40.00]
          · text [404.00 444.00 44.00x22.00] "chip 5" clip [0.00 440.00 640.00x40.00]
    AnimatedPosition [480.00 440.00 80.00x30.00] content 80.00x30.00 clip [0.00 440.00 640.00x40.00]
      Container [480.00 440.00 80.00x30.00] content 52.00x30.00 clip [0.00 440.00 640.00x40.00]
        Text [484.00 444.00 44.00x22.00] content 44.00x21.00 clip [0.00 440.00 640.00x40.00]
          · text [484.00 444.00 44.00x22.00] "chip 6" clip [0.00 440.00 640.00x40.00]
    AnimatedPosition [560.00 440.00 80.00x30.00] content 80.00x30.00 clip [0.00 440.00 640.00x40.00]
      Container [560.00 440.00 80.00x30.00] content 51.00x30.00 clip [0.00 440.00 640.00x40.00]
        Text [564.00 444.00 43.00x22.00] content 43.00x21.00 clip [0.00 440.00 640.00x40.00]
          · text [564.00 444.00 43.00x22.00] "chip 7" clip [0.00 440.00 640.00x40.00]
    AnimatedPosition [640.00 440.00 80.00x30.00] content 80.00x30.00 clip [0.00 440.00 640.00x40.00]
      Container [640.00 440.00 80.00x30.00] content 52.00x30.00 clip [0.00 440.00 640.00x40.00]
        Text [644.00 444.00 44.00x22.00] content 44.00x21.00 clip [0.00 440.00 640.00x40.00]
          · text [644.00 444.00 44.00x22.00] "chip 8" clip [0.00 440.00 640.00x40.00]
    AnimatedPosition [720.00 440.00 80.00x30.00] content 80.00x30.00 culled
      Container [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
        Text [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
    AnimatedPosition [800.00 440.00 80.00x30.00] content 80.00x30.00 culled
      Container [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
        Text [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
    AnimatedPosition [880.00 440.00 80.00x30.00] content 80.00x30.00 culled
      Container [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
        Text [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
    AnimatedPosition [960.00 440.00 80.00x30.00] content 80.00x30.00 culled
      Container [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
        Text [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
    AnimatedPosition [1040.00 440.00 80.00x30.00] content 80.00x30.00 culled
      Container [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
        Text [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
    AnimatedPosition [1120.00 440.00 80.00x30.00] content 80.00x30.00 culled
      Container [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
        Text [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
    AnimatedPosition [1200.00 440.00 80.00x30.00] content 80.00x30.00 culled
      Container [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
        Text [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
    AnimatedPosition [1280.00 440.00 80.00x30.00] content 80.00x30.00 culled
      Container [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
        Text [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
=== scrolling
Scrollable [0.00 0.00 640.00x480.00] content 630.00x840.00
  · quad [633.35 2.00 3.30x272.00] clip [0.00 0.00 640.00x480.00]
  Column [0.00 0.00 630.00x840.00] content 630.00x840.00 clip [0.00 0.00 640.00x480.00]
    Row [0.00 0.00 630.00x21.00] content 630.00x21.00 clip [0.00 0.00 640.00x480.00]
      Text [0.00 0.00 62.00x21.00] content 62.00x21.00 clip [0.00 0.00 640.00x480.00]
        · text [0.00 0.00 62.00x21.00] "setting 0" clip [0.00 0.00 640.00x480.00]
      Spacer [62.00 0.00 529.00x21.00] content 0.00x0.00 clip [0.00 0.00 640.00x480.00]
      Text [591.00 0.00 39.00x21.00] content 39.00x21.00 clip [0.00 0.00 640.00x480.00]
        · text [591.00 0.00 39.00x21.00] "value" clip [0.00 0.00 640.00x480.00]
    Row [0.00 21.00 630.00x21.00] content 630.00x21.00 clip [0.00 0.00 640.00x480.00]
      Text [0.00 21.00 59.00x21.00] content 59.00x21.00 clip [0.00 0.00 640.00x480.00]
        · text [0.00 21.00 59.00x21.00] "setting 1" clip [0.00 0.00 640.00x480.00]
      Spacer [59.00 21.00 532.00x21.00] content 0.00x0.00 clip [0.00 0.00 640.00x480.00]
      Text [591.00 21.00 39.00x21.00] content 39.00x21.00 clip [0.00 0.00 640.00x480.00]
        · text [591.00 21.00 39.00x21.00] "value" clip [0.00 0.00 640.00x480.00]
    Row [0.00 42.00 630.00x21.00] content 630.00x21.00 clip [0.00 0.00 640.00x480.00]
      Text [0.00 42.00 62.00x21.00] content 62.00x21.00 clip [0.00 0.00 640.00x480.00]
        · text [0.00 42.00 62.00x21.00] "setting 2" clip [0.00 0.00 640.00x480.00]
      Spacer [62.00 42.00 529.00x21.00] content 0.00x0.00 clip [0.00 0.00 640.00x480.00]
      Text [591.00 42.00 39.00x21.00] content 39.00x21.00 clip [0.00 0.00 640.00x480.00]
        · text [591.00 42.00 39.00x21.00] "value" clip [0.00 0.00 640.00x480.00]
    Row [0.00 63.00 630.00x21.00] content 630.00x21.00 clip [0.00 0.00 640.00x480.00]
      Text [0.00 63.00 62.00x21.00] content 62.00x21.00 clip [0.00 0.00 640.00x480.00]
        · text [0.00 63.00 62.00x21.00] "setting 3" clip [0.00 0.00 640.00x480.00]
      Spacer [62.00 63.00 529.00x21.00] content 0.00x0.00 clip [0.00 0.00 640.00x480.00]
      Text [591.00 63.00 39.00x21.00] content 39.00x21.00 clip [0.00 0.00 640.00x480.00]
        · text [591.00 63.00 39.00x21.00] "value" clip [0.00 0.00 640.00x480.00]
    Row [0.00 84.00 630.00x21.00] content 630.00x21.00 clip [0.00 0.00 640.00x480.00]
      Text [0.00 84.00 63.00x21.00] content 63.00x21.00 clip [0.00 0.00 640.00x480.00]
        · text [0.00 84.00 63.00x21.00] "setting 4" clip [0.00 0.00 640.00x480.00]
      Spacer [63.00 84.00 528.00x21.00] content 0.00x0.00 clip [0.00 0.00 640.00x480.00]
      Text [591.00 84.00 39.00x21.00] content 39.00x21.00 clip [0.00 0.00 640.00x480.00]
        · text [591.00 84.00 39.00x21.00] "value" clip [0.00 0.00 640.00x480.00]
    Row [0.00 105.00 630.00x21.00] content 630.00x21.00 clip [0.00 0.00 640.00x480.00]
      Text [0.00 105.00 62.00x21.00] content 62.00x21.00 clip [0.00 0.00 640.00x480.00]
        · text [0.00 105.00 62.00x21.00] "setting 5" clip [0.00 0.00 640.00x480.00]
      Spacer [62.00 105.00 529.00x21.00] content 0.00x0.00 clip [0.00 0.00 640.00x480.00]
      Text [591.00 105.00 39.00x21.00] content 39.00x21.00 clip [0.00 0.00 640.00x480.00]
        · text [591.00 105.00 39.00x21.00] "value" clip [0.00 0.00 640.00x480.00]
    Row [0.00 126.00 630.00x21.00] content 630.00x21.00 clip [0.00 0.00 640.00x480.00]
      Text [0.00 126.00 62.00x21.00] content 62.00x21.00 clip [0.00 0.00 640.00x480.00]
        · text [0.00 126.00 62.00x21.00] "setting 6" clip [0.00 0.00 640.00x480.00]
      Spacer [62.00 126.00 529.00x21.00] content 0.00x0.00 clip [0.00 0.00 640.00x480.00]
      Text [591.00 126.00 39.00x21.00] content 39.00x21.00 clip [0.00 0.00 640.00x480.00]
        · text [591.00 126.00 39.00x21.00] "value" clip [0.00 0.00 640.00x480.00]
    Row [0.00 147.00 630.00x21.00] content 630.00x21.00 clip [0.00 0.00 640.00x480.00]
      Text [0.00 147.00 61.00x21.00] content 61.00x21.00 clip [0.00 0.00 640.00x480.00]
        · text [0.00 147.00 61.00x21.00] "setting 7" clip [0.00 0.00 640.00x480.00]
      Spacer [61.00 147.00 530.00x21.00] content 0.00x0.00 clip [0.00 0.00 640.00x480.00]
      Text [591.00 147.00 39.00x21.00] content 39.00x21.00 clip [0.00 0.00 640.00x480.00]
        · text [591.00 147.00 39.00x21.00] "value" clip [0.00 0.00 640.00x480.00]
    Row [0.00 168.00 630.00x21.00] content 630.00x21.00 clip [0.00 0.00 640.00x480.00]
      Text [0.00 168.00 62.00x21.00] content 62.00x21.00 clip [0.00 0.00 640.00x480.00]
        · text [0.00 168.00 62.00x21.00] "setting 8" clip [0.00 0.00 640.00x480.00]
      Spacer [62.00 168.00 529.00x21.00] content 0.00x0.00 clip [0.00 0.00 640.00x480.00]
      Text [591.00 168.00 39.00x21.00] content 39.00x21.00 clip [0.00 0.00 640.00x480.00]
        · text [591.00 168.00 39.00x21.00] "value" clip [0.00 0.00 640.00x480.00]
    Row [0.00 189.00 630.00x21.00] content 630.00x21.00 clip [0.00 0.00 640.00x480.00]
      Text [0.00 189.00 62.00x21.00] content 62.00x21.00 clip [0.00 0.00 640.00x480.00]
        · text [0.00 189.00 62.00x21.00] "setting 9" clip [0.00 0.00 640.00x480.00]
      Spacer [62.00 189.00 529.00x21.00] content 0.00x0.00 clip [0.00 0.00 640.00x480.00]
      Text [591.00 189.00 39.00x21.00] content 39.00x21.00 clip [0.00 0.00 640.00x480.00]
        · text [591.00 189.00 39.00x21.00] "value" clip [0.00 0.00 640.00x480.00]
    Row [0.00 210.00 630.00x21.00] content 630.00x21.00 clip [0.00 0.00 640.00x480.00]
      Text [0.00 210.00 68.00x21.00] content 68.00x21.00 clip [0.00 0.00 640.00x480.00]
        · text [0.00 210.00 68.00x21.00] "setting 10" clip [0.00 0.00 640.00x480.00]
      Spacer [68.00 210.00 523.00x21.00] content 0.00x0.00 clip [0.00 0.00 640.00x480.00]
      Text [591.00 210.00 39.00x21.00] content 39.00x21.00 clip [0.00 0.00 640.00x480.00]
        · text [591.00 210.00 39.00x21.00] "value" clip [0.00 0.00 640.00x480.00]
    Row [0.00 231.00 630.00x21.00] content 630.00x21.00 clip [0.00 0.00 640.00x480.00]
      Text [0.00 231.00 65.00x21.00] content 65.00x21.00 clip [0.00 0.00 640.00x480.00]
        · text [0.00 231.00 65.00x21.00] "setting 11" clip [0.00 0.00 640.00x480.00]
      Spacer [65.00 231.00 526.00x21.00] content 0.00x0.00 clip [0.00 0.00 640.00x480.00]
      Text [591.00 231.00 39.00x21.00] content 39.00x21.00 clip [0.00 0.00 640.00x480.00]
        · text [591.00 231.00 39.00x21.00] "value" clip [0.00 0.00 640.00x480.00]
    Row [0.00 252.00 630.00x21.00] content 630.00x21.00 clip [0.00 0.00 640.00x480.00]
      Text [0.00 252.00 68.00x21.00] content 68.00x21.00 clip [0.00 0.00 640.00x480.00]
        · text [0.00 252.00 68.00x21.00] "setting 12" clip [0.00 0.00 640.00x480.00]
      Spacer [68.00 252.00 523.00x21.00] content 0.00x0.00 clip [0.00 0.00 640.00x480.00]
      Text [591.00 252.00 39.00x21.00] content 39.00x21.00 clip [0.00 0.00 640.00x480.00]
        · text [591.00 252.00 39.00x21.00] "value" clip [0.00 0.00 640.00x480.00]
    Row [0.00 273.00 630.00x21.00] content 630.00x21.00 clip [0.00 0.00 640.00x480.00]
      Text [0.00 273.00 68.00x21.00] content 68.00x21.00 clip [0.00 0.00 640.00x480.00]
        · text [0.00 273.00 68.00x21.00] "setting 13" clip [0.00 0.00 640.00x480.00]
      Spacer [68.00 273.00 523.00x21.00] content 0.00x0.00 clip [0.00 0.00 640.00x480.00]
      Text [591.00 273.00 39.00x21.00] content 39.00x21.00 clip [0.00 0.00 640.00x480.00]
        · text [591.00 273.00 39.00x21.00] "value" clip [0.00 0.00 640.00x480.00]
    Row [0.00 294.00 630.00x21.00] content 630.00x21.00 clip [0.00 0.00 640.00x480.00]
      Text [0.00 294.00 69.00x21.00] content 69.00x21.00 clip [0.00 0.00 640.00x480.00]
        · text [0.00 294.00 69.00x21.00] "setting 14" clip [0.00 0.00 640.00x480.00]
      Spacer [69.00 294.00 522.00x21.00] content 0.00x0.00 clip [0.00 0.00 640.00x480.00]
      Text [591.00 294.00 39.00x21.00] content 39.00x21.00 clip [0.00 0.00 640.00x480.00]
        · text [591.00 294.00 39.00x21.00] "value" clip [0.00 0.00 640.00x480.00]
    Row [0.00 315.00 630.00x21.00] content 630.00x21.00 clip [0.00 0.00 640.00x480.00]
      Text [0.00 315.00 68.00x21.00] content 68.00x21.00 clip [0.00 0.00 640.00x480.00]
        · text [0.00 315.00 68.00x21.00] "setting 15" clip [0.00 0.00 640.00x480.00]
      Spacer [68.00 315.00 523.00x21.00] content 0.00x0.00 clip [0.00 0.00 640.00x480.00]
      Text [591.00 315.00 39.00x21.00] content 39.00x21.00 clip [0.00 0.00 640.00x480.00]
        · text [591.00 315.00 39.00x21.00] "value" clip [0.00 0.00 640.00x480.00]
    Row [0.00 336.00 630.00x21.00] content 630.00x21.00 clip [0.00 0.00 640.00x480.00]
      Text [0.00 336.00 68.00x21.00] content 68.00x21.00 clip [0.00 0.00 640.00x480.00]
        · text [0.00 336.00 68.00x21.00] "setting 16" clip [0.00 0.00 640.00x480.00]
      Spacer [68.00 336.00 523.00x21.00] content 0.00x0.00 clip [0.00 0.00 640.00x480.00]
      Text [591.00 336.00 39.00x21.00] content 39.00x21.00 clip [0.00 0.00 640.00x480.00]
        · text [591.00 336.00 39.00x21.00] "value" clip [0.00 0.00 640.00x480.00]
    Row [0.00 357.00 630.00x21.00] content 630.00x21.00 clip [0.00 0.00 640.00x480.00]
      Text [0.00 357.00 67.00x21.00] content 67.00x21.00 clip [0.00 0.00 640.00x480.00]
        · text [0.00 357.00 67.00x21.00] "setting 17" clip [0.00 0.00 640.00x480.00]
      Spacer [67.00 357.00 524.00x21.00] content 0.00x0.00 clip [0.00 0.00 640.00x480.00]
      Text [591.00 357.00 39.00x21.00] content 39.00x21.00 clip [0.00 0.00 640.00x480.00]
        · text [591.00 357.00 39.00x21.00] "value" clip [0.00 0.00 640.00x480.00]
    Row [0.00 378.00 630.00x21.00] content 630.00x21.00 clip [0.00 0.00 640.00x480.00]
      Text [0.00 378.00 68.00x21.00] content 68.00x21.00 clip [0.00 0.00 640.00x480.00]
        · text [0.00 378.00 68.00x21.00] "setting 18" clip [0.00 0.00 640.00x480.00]
      Spacer [68.00 378.00 523.00x21.00] content 0.00x0.00 clip [0.00 0.00 640.00x480.00]
      Text [591.00 378.00 39.00x21.00] content 39.00x21.00 clip [0.00 0.00 640.00x480.00]
        · text [591.00 378.00 39.00x21.00] "value" clip [0.00 0.00 640.00x480.00]
    Row [0.00 399.00 630.00x21.00] content 630.00x21.00 clip [0.00 0.00 640.00x480.00]
      Text [0.00 399.00 68.00x21.00] content 68.00x21.00 clip [0.00 0.00 640.00x480.00]
        · text [0.00 399.00 68.00x21.00] "setting 19" clip [0.00 0.00 640.00x480.00]
      Spacer [68.00 399.00 523.00x21.00] content 0.00x0.00 clip [0.00 0.00 640.00x480.00]
      Text [591.00 399.00 39.00x21.00] content 39.00x21.00 clip [0.00 0.00 640.00x480.00]
        · text [591.00 399.00 39.00x21.00] "value" clip [0.00 0.00 640.00x480.00]
    Row [0.00 420.00 630.00x21.00] content 630.00x21.00 clip [0.00 0.00 640.00x480.00]
      Text [0.00 420.00 72.00x21.00] content 72.00x21.00 clip [0.00 0.00 640.00x480.00]
        · text [0.00 420.00 72.00x21.00] "setting 20" clip [0.00 0.00 640.00x480.00]
      Spacer [72.00 420.00 519.00x21.00] content 0.00x0.00 clip [0.00 0.00 640.00x480.00]
      Text [591.00 420.00 39.00x21.00] content 39.00x21.00 clip [0.00 0.00 640.00x480.00]
        · text [591.00 420.00 39.00x21.00] "value" clip [0.00 0.00 640.00x480.00]
    Row [0.00 441.00 630.00x21.00] content 630.00x21.00 clip [0.00 0.00 640.00x480.00]
      Text [0.00 441.00 68.00x21.00] content 68.00x21.00 clip [0.00 0.00 640.00x480.00]
        · text [0.00 441.00 68.00x21.00] "setting 21" clip [0.00 0.00 640.00x480.00]
      Spacer [68.00 441.00 523.00x21.00] content 0.00x0.00 clip [0.00 0.00 640.00x480.00]
      Text [591.00 441.00 39.00x21.00] content 39.00x21.00 clip [0.00 0.00 640.00x480.00]
        · text [591.00 441.00 39.00x21.00] "value" clip [0.00 0.00 640.00x480.00]
    Row [0.00 462.00 630.00x21.00] content 630.00x21.00 clip [0.00 0.00 640.00x480.00]
      Text [0.00 462.00 71.00x21.00] content 71.00x21.00 clip [0.00 0.00 640.00x480.00]
        · text [0.00 462.00 71.00x21.00] "setting 22" clip [0.00 0.00 640.00x480.00]
      Spacer [71.00 462.00 520.00x21.00] content 0.00x0.00 clip [0.00 0.00 640.00x480.00]
      Text [591.00 462.00 39.00x21.00] content 39.00x21.00 clip [0.00 0.00 640.00x480.00]
        · text [591.00 462.00 39.00x21.00] "value" clip [0.00 0.00 640.00x480.00]
    Row [0.00 483.00 630.00x21.00] content 630.00x21.00 clip [0.00 0.00 640.00x480.00]
      Text [0.00 483.00 71.00x21.00] content 71.00x21.00 clip [0.00 0.00 640.00x480.00]
        · text [0.00 483.00 71.00x21.00] "setting 23" clip [0.00 0.00 640.00x480.00]
      Spacer [71.00 483.00 520.00x21.00] content 0.00x0.00 clip [0.00 0.00 640.00x480.00]
      Text [591.00 483.00 39.00x21.00] content 39.00x21.00 clip [0.00 0.00 640.00x480.00]
        · text [591.00 483.00 39.00x21.00] "value" clip [0.00 0.00 640.00x480.00]
    Row [0.00 504.00 630.00x21.00] content 630.00x21.00 clip [0.00 0.00 640.00x480.00]
      Text [0.00 504.00 71.00x21.00] content 71.00x21.00 clip [0.00 0.00 640.00x480.00]
        · text [0.00 504.00 71.00x21.00] "setting 24" clip [0.00 0.00 640.00x480.00]
      Spacer [71.00 504.00 520.00x21.00] content 0.00x0.00 clip [0.00 0.00 640.00x480.00]
      Text [591.00 504.00 39.00x21.00] content 39.00x21.00 clip [0.00 0.00 640.00x480.00]
        · text [591.00 504.00 39.00x21.00] "value" clip [0.00 0.00 640.00x480.00]
    Row [0.00 525.00 630.00x21.00] content 630.00x21.00 clip [0.00 0.00 640.00x480.00]
      Text [0.00 525.00 71.00x21.00] content 71.00x21.00 clip [0.00 0.00 640.00x480.00]
        · text [0.00 525.00 71.00x21.00] "setting 25" clip [0.00 0.00 640.00x480.00]
      Spacer [71.00 525.00 520.00x21.00] content 0.00x0.00 clip [0.00 0.00 640.00x480.00]
      Text [591.00 525.00 39.00x21.00] content 39.00x21.00 clip [0.00 0.00 640.00x480.00]
        · text [591.00 525.00 39.00x21.00] "value" clip [0.00 0.00 640.00x480.00]
    Row [0.00 546.00 630.00x21.00] content 630.00x21.00 culled
      Text [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
      Spacer [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
      Text [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
    Row [0.00 567.00 630.00x21.00] content 630.00x21.00 culled
      Text [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
      Spacer [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
      Text [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
    Row [0.00 588.00 630.00x21.00] content 630.00x21.00 culled
      Text [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
      Spacer [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
      Text [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
    Row [0.00 609.00 630.00x21.00] content 630.00x21.00 culled
      Text [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
      Spacer [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
      Text [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
    Row [0.00 630.00 630.00x21.00] content 630.00x21.00 culled
      Text [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
      Spacer [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
      Text [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
    Row [0.00 651.00 630.00x21.00] content 630.00x21.00 culled
      Text [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
      Spacer [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
      Text [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
    Row [0.00 672.00 630.00x21.00] content 630.00x21.00 culled
      Text [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
      Spacer [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
      Text [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
    Row [0.00 693.00 630.00x21.00] content 630.00x21.00 culled
      Text [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
      Spacer [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
      Text [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
    Row [0.00 714.00 630.00x21.00] content 630.00x21.00 culled
      Text [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
      Spacer [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
      Text [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
    Row [0.00 735.00 630.00x21.00] content 630.00x21.00 culled
      Text [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
      Spacer [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
      Text [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
    Row [0.00 756.00 630.00x21.00] content 630.00x21.00 culled
      Text [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
      Spacer [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
      Text [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
    Row [0.00 777.00 630.00x21.00] content 630.00x21.00 culled
      Text [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
      Spacer [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
      Text [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
    Row [0.00 798.00 630.00x21.00] content 630.00x21.00 culled
      Text [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
      Spacer [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
      Text [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
    Row [0.00 819.00 630.00x21.00] content 630.00x21.00 culled
      Text [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
      Spacer [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
      Text [0.00 0.00 0.00x0.00] content 0.00x0.00 culled
=== settings
Scrollable [0.00 0.00 640.00x480.00] content 640.00x401.00
  Column [0.00 0.00 640.00x401.00] content 640.00x401.00 clip [0.00 0.00 640.00x480.00]
    Container [16.00 16.00 608.00x190.00] content 608.00x190.00 clip [0.00 0.00 640.00x480.00]
      · quad [16.00 16.00 608.00x190.00] clip [0.00 0.00 640.00x480.00]
      Column [32.00 32.00 576.00x158.00] content 576.00x158.00 clip [0.00 0.00 640.00x480.00]
        Text [32.00 32.00 576.00x24.00] content 102.00x24.00 clip [0.00 0.00 640.00x480.00]
          · text [32.00 32.00 576.00x24.00] "Appearance" clip [0.00 0.00 640.00x480.00]
        Container [32.00 68.00 576.00x122.00] content 290.00x122.00 clip [0.00 0.00 640.00x480.00]
          · quad [32.00 68.00 576.00x122.00] clip [0.00 0.00 640.00x480.00]
          Column [48.00 84.00 258.00x90.00] content 258.00x90.00 clip [0.00 0.00 640.00x480.00]
            Row [48.00 84.00 258.00x36.00] content 258.00x36.00 clip [0.00 0.00 640.00x480.00]
              Text [48.00 92.00 42.00x21.00] content 42.00x21.00 clip [0.00 0.00 640.00x480.00]
                · text [48.00 92.00 42.00x21.00] "Name" clip [0.00 0.00 640.00x480.00]
              Container [106.00 84.00 200.00x36.00] content 200.00x36.00 clip [0.00 0.00 640.00x480.00]
                TextField [106.00 84.00 200.00x36.00] content 24.00x0.00 clip [0.00 0.00 640.00x480.00]
                  · quad [106.00 84.00 200.00x36.00] clip [106.00 84.00 200.00x36.00]
                  · text [118.00 92.00 176.00x22.00] "Rikka" clip [118.00 84.00 176.00x36.00]
            Row [48.00 120.00 258.00x28.00] content 258.00x28.00 clip [0.00 0.00 640.00x480.00]
              Text [48.00 124.00 34.00x21.00] content 34.00x21.00 clip [0.00 0.00 640.00x480.00]
                · text [48.00 124.00 34.00x21.00] "Dark" clip [0.00 0.00 640.00x480.00]
              Container [98.00 120.00 208.00x28.00] content 208.00x28.00 clip [0.00 0.00 640.00x480.00]
                Switch [260.00 120.00 46.00x28.00] content 46.00x28.00 clip [0.00 0.00 640.00x480.00]
                  · quad [260.00 120.00 46.00x28.00] clip [0.00 0.00 640.00x480.00]
                  · quad [281.00 123.00 22.00x22.00] clip [0.00 0.00 640.00x480.00]
            Row [48.00 148.00 258.00x26.00] content 258.00x26.00 clip [0.00 0.00 640.00x480.00]
              Text [48.00 151.00 40.00x21.00] content 40.00x21.00 clip [0.00 0.00 640.00x480.00]
                · text [48.00 151.00 40.00x21.00] "Scale" clip [0.00 0.00 640.00x480.00]
              Container [104.00 148.00 202.00x26.00] content 202.00x26.00 clip [0.00 0.00 640.00x480.00]
                Slider [106.00 148.00 200.00x26.00] content 0.00x0.00 clip [0.00 0.00 640.00x480.00]
                  · quad [106.00 158.00 200.00x6.00] clip [0.00 0.00 640.00x480.00]
                  · quad [106.00 158.00 100.00x6.00] clip [0.00 0.00 640.00x480.00]
                  · quad [193.00 148.00 26.00x26.00] clip [0.00 0.00 640.00x480.00]
    Container [16.00 222.00 608.00x163.00] content 608.00x163.00 clip [0.00 0.00 640.00x480.00]
      · quad [16.00 222.00 608.00x163.00] clip [0.00 0.00 640.00x480.00]
      Column [32.00 238.00 576.00x131.00] content 576.00x131.00 clip [0.00 0.00 640.00x480.00]
        Text [32.00 238.00 576.00x24.00] content 51.00x24.00 clip [0.00 0.00 640.00x480.00]
          · text [32.00 238.00 576.00x24.00] "About" clip [0.00 0.00 640.00x480.00]
        Container [32.00 274.00 576.00x95.00] content 576.00x95.00 clip [0.00 0.00 640.00x480.00]
          · quad [32.00 274.00 576.00x95.00] clip [0.00 0.00 640.00x480.00]
          Text [48.00 290.00 544.00x63.00] content 540.00x63.00 clip [0.00 0.00 640.00x480.00]
            · text [48.00 290.00 544.00x63.00] "A layout is only as good as the rules it keeps: text wraps inside the room it is given, labels that must stay on one line give way with an ellipsis, and rows that grow share what is left between them." clip [0.00 0.00 640.00x480.00]
=== menu
Column [0.00 0.00 640.00x480.00] content 640.00x146.00
  Button [16.00 16.00 608.00x36.00] content 340.00x29.00
    · quad [16.00 16.00 608.00x36.00]
    Text [301.00 24.00 39.00x21.00] content 39.00x21.00
      · text [301.00 24.00 39.00x21.00] "Open"
  Menu [16.00 56.00 200.00x90.00] content 0.00x0.00
    · quad [16.00 56.00 200.00x90.00]
    · text [30.00 64.46 172.00x21.00] "Copy" clip [30.00 62.00 172.00x25.92]
    · text [30.00 90.38 172.00x21.00] "Paste" clip [30.00 87.92 172.00x25.92]
    · icon [187.00 119.30 15.00x15.00] clip [16.00 56.00 200.00x90.00]
    · text [30.00 116.30 149.00x21.00] "SplitRatios" clip [30.00 113.84 149.00x25.92]
=== panes
Split [0.00 0.00 640.00x480.00] content 640.00x480.00
  · quad [192.70 0.00 1.00x480.00]
  SplitPart [0.00 0.00 190.00x480.00] content 190.00x480.00
    Pane [0.00 0.00 190.00x480.00] content 190.00x212.00
      Column [0.00 0.00 190.00x212.00] content 190.00x212.00
        Tabs [0.00 0.00 190.00x44.00] content 0.00x0.00
          · quad [0.00 0.00 190.00x44.00]
          · quad [40.00 4.00 102.00x36.00] clip [40.00 4.00 110.00x36.00]
          · text [48.00 11.50 86.00x21.00] "One" clip [48.00 4.00 86.00x36.00]
          · icon [13.00 13.00 18.00x18.00]
          · icon [159.00 13.00 18.00x18.00]
        Text [0.00 44.00 190.00x168.00] content 190.00x168.00
          · text [0.00 44.00 190.00x168.00] "A layout is only as good as the rules it keeps: text wraps inside the room it is given, labels that must stay on one line give way with an ellipsis, and rows that grow share what is left between them."
  SplitPart [196.00 0.00 444.00x480.00] content 444.00x480.00
    Pane [196.00 0.00 444.00x480.00] content 444.00x114.00
      Column [196.00 0.00 444.00x114.00] content 444.00x114.00
        Text [196.00 0.00 444.00x30.00] content 134.00x30.00
          · text [196.00 0.00 444.00x30.00] "second pane"
        Text [196.00 30.00 444.00x84.00] content 444.00x84.00
          · text [196.00 30.00 444.00x84.00] "A layout is only as good as the rules it keeps: text wraps inside the room it is given, labels that must stay on one line give way with an ellipsis, and rows that grow share what is left between them."
=== document
Scrollable [0.00 0.00 640.00x480.00] content 640.00x359.00
  Column [0.00 0.00 640.00x359.00] content 640.00x359.00 clip [0.00 0.00 640.00x480.00]
    Column [0.00 0.00 640.00x45.00] content 640.00x45.00 clip [0.00 0.00 640.00x480.00]
      RichText [0.00 0.00 640.00x38.00] content 123.00x38.00 clip [0.00 0.00 640.00x480.00]
        · text [0.00 0.00 640.00x38.00] "Heading" clip [0.00 0.00 640.00x480.00]
      Container [0.00 44.00 640.00x1.00] content 0.00x0.00 clip [0.00 0.00 640.00x480.00]
        · quad [0.00 44.00 640.00x1.00] clip [0.00 0.00 640.00x480.00]
        Spacer [0.00 44.00 0.00x1.00] content 0.00x0.00 clip [0.00 0.00 640.00x480.00]
    RichText [0.00 59.00 640.00x48.00] content 606.00x48.00 clip [0.00 0.00 640.00x480.00]
      · quad [146.13 61.62 37.12x18.75] clip [0.00 0.00 640.00x480.00]
      · text [0.00 59.00 640.00x48.00] "Some emphasis and code in a paragraph long enough to wrap across the width of the window." clip [0.00 0.00 640.00x480.00]
    Column [0.00 121.00 640.00x81.00] content 640.00x81.00 clip [0.00 0.00 640.00x480.00]
      Row [0.00 121.00 640.00x24.00] content 640.00x24.00 clip [0.00 0.00 640.00x480.00]
        Container [0.00 121.00 19.00x24.00] content 19.00x24.00 clip [0.00 0.00 640.00x480.00]
          RichText [10.00 121.00 9.00x24.00] content 9.00x24.00 clip [0.00 0.00 640.00x480.00]
            · text [10.00 121.00 9.00x24.00] "•" clip [0.00 0.00 640.00x480.00]
        Column [24.00 121.00 616.00x24.00] content 616.00x24.00 clip [0.00 0.00 640.00x480.00]
          RichText [24.00 121.00 616.00x24.00] content 27.00x24.00 clip [0.00 0.00 640.00x480.00]
            · text [24.00 121.00 616.00x24.00] "one" clip [0.00 0.00 640.00x480.00]
      Row [0.00 150.00 640.00x24.00] content 640.00x24.00 clip [0.00 0.00 640.00x480.00]
        Container [0.00 150.00 19.00x24.00] content 19.00x24.00 clip [0.00 0.00 640.00x480.00]
          RichText [10.00 150.00 9.00x24.00] content 9.00x24.00 clip [0.00 0.00 640.00x480.00]
            · text [10.00 150.00 9.00x24.00] "•" clip [0.00 0.00 640.00x480.00]
        Column [24.00 150.00 616.00x24.00] content 616.00x24.00 clip [0.00 0.00 640.00x480.00]
          RichText [24.00 150.00 616.00x24.00] content 27.00x24.00 clip [0.00 0.00 640.00x480.00]
            · text [24.00 150.00 616.00x24.00] "two" clip [0.00 0.00 640.00x480.00]
      Row [0.00 178.00 640.00x24.00] content 640.00x24.00 clip [0.00 0.00 640.00x480.00]
        Container [0.00 178.00 19.00x24.00] content 19.00x24.00 clip [0.00 0.00 640.00x480.00]
          RichText [10.00 178.00 9.00x24.00] content 9.00x24.00 clip [0.00 0.00 640.00x480.00]
            · text [10.00 178.00 9.00x24.00] "•" clip [0.00 0.00 640.00x480.00]
        Column [24.00 178.00 616.00x24.00] content 616.00x24.00 clip [0.00 0.00 640.00x480.00]
          RichText [24.00 178.00 616.00x24.00] content 37.00x24.00 clip [0.00 0.00 640.00x480.00]
            · text [24.00 178.00 616.00x24.00] "three" clip [0.00 0.00 640.00x480.00]
    Container [0.00 216.00 640.00x105.00] content 640.00x105.00 clip [0.00 0.00 640.00x480.00]
      · quad [0.00 216.00 640.00x105.00] clip [0.00 0.00 640.00x480.00]
      Column [0.00 216.00 640.00x105.00] content 640.00x105.00 clip [0.00 0.00 640.00x480.00]
        Row [0.00 216.00 640.00x34.00] content 640.00x34.00 clip [0.00 0.00 640.00x480.00]
          Text [11.00 225.00 34.00x20.00] content 34.00x20.00 clip [0.00 0.00 640.00x480.00]
            · text [11.00 225.00 34.00x20.00] "rust" clip [0.00 0.00 640.00x480.00]
          Spacer [45.00 235.00 429.00x0.00] content 0.00x0.00 clip [0.00 0.00 640.00x480.00]
          Button [474.00 220.00 162.00x30.00] content 162.00x24.00 clip [0.00 0.00 640.00x480.00]
            · quad [474.00 220.00 162.00x30.00] clip [0.00 0.00 640.00x480.00]
            Row [486.00 226.00 138.00x18.00] content 138.00x18.00 clip [0.00 0.00 640.00x480.00]
              Icon [486.00 227.00 16.00x16.00] content 16.00x16.00 clip [0.00 0.00 640.00x480.00]
                · icon [486.00 227.00 16.00x16.00] clip [0.00 0.00 640.00x480.00]
              Text [506.00 226.00 118.00x18.00] content 118.00x18.00 clip [0.00 0.00 640.00x480.00]
                · text [506.00 226.00 118.00x18.00] "widgets.copy.copy" clip [0.00 0.00 640.00x480.00]
        Container [0.00 250.00 640.00x71.00] content 640.00x71.00 clip [0.00 0.00 640.00x480.00]
          RichText [11.00 250.00 618.00x60.00] content 185.00x60.00 clip [0.00 0.00 640.00x480.00]
            · text [11.00 250.00 618.00x60.00] "fn main() {\n    println!(\"hello\");\n}" clip [0.00 0.00 640.00x480.00]
    Row [0.00 335.00 640.00x24.00] content 640.00x24.00 clip [0.00 0.00 640.00x480.00]
      Container [0.00 335.00 3.00x24.00] content 3.00x24.00 clip [0.00 0.00 640.00x480.00]
        · quad [0.00 335.00 3.00x24.00] clip [0.00 0.00 640.00x480.00]
        Spacer [0.00 335.00 3.00x24.00] content 0.00x0.00 clip [0.00 0.00 640.00x480.00]
      Column [14.00 335.00 626.00x24.00] content 626.00x24.00 clip [0.00 0.00 640.00x480.00]
        RichText [14.00 335.00 626.00x24.00] content 509.00x24.00 clip [0.00 0.00 640.00x480.00]
          · text [14.00 335.00 626.00x24.00] "A quote that is also long enough to need a second line when it is drawn." clip [0.00 0.00 640.00x480.00]
=== editors
Column [0.00 0.00 640.00x480.00] content 640.00x480.00
  TextEditor [8.00 8.00 624.00x62.00] content 175.00x62.00
    · text [45.00 16.00 575.00x47.00] "a reply\nwith two lines" clip [45.00 8.00 575.00x62.00]
    · text [20.00 16.00 9.00x47.00] "1\n2" clip [8.00 8.00 624.00x62.00]
  TextEditor [8.00 78.00 624.00x394.00] content 0.00x0.00
    · text [45.00 86.00 575.00x70.00] "first line\nsecond line that is rather longer than the first one\nthird" clip [45.00 78.00 575.00x394.00]
    · text [20.00 86.00 9.00x70.00] "1\n2\n3" clip [8.00 78.00 624.00x394.00]
=== blocks
Column [0.00 0.00 640.00x480.00] content 640.00x313.00
  Toolbar [8.00 8.00 624.00x44.00] content 183.00x44.00
    Slot [16.00 12.00 167.00x36.00] content 167.00x36.00 clip [8.00 8.00 624.00x44.00]
      Row [16.00 12.00 167.00x36.00] content 167.00x36.00 clip [8.00 8.00 624.00x44.00]
        Button [16.00 12.00 68.00x36.00] content 68.00x29.00 clip [8.00 8.00 624.00x44.00]
          · quad [16.00 12.00 68.00x36.00] clip [8.00 8.00 624.00x44.00]
          Text [32.00 20.00 36.00x21.00] content 36.00x21.00 clip [8.00 8.00 624.00x44.00]
            · text [32.00 20.00 36.00x21.00] "Back" clip [8.00 8.00 624.00x44.00]
        Button [92.00 12.00 91.00x36.00] content 91.00x29.00 clip [8.00 8.00 624.00x44.00]
          · quad [92.00 12.00 91.00x36.00] clip [8.00 8.00 624.00x44.00]
          Text [108.00 20.00 59.00x21.00] content 59.00x21.00 clip [8.00 8.00 624.00x44.00]
            · text [108.00 20.00 59.00x21.00] "Forward" clip [8.00 8.00 624.00x44.00]
  Grid [8.00 60.00 624.00x87.00] content 208.00x29.00
    Container [8.00 60.00 208.00x29.00] content 50.00x29.00
      Text [12.00 64.00 42.00x21.00] content 42.00x21.00
        · text [12.00 64.00 42.00x21.00] "grid 0"
    Container [216.00 60.00 208.00x29.00] content 47.00x29.00
      Text [220.00 64.00 39.00x21.00] content 39.00x21.00
        · text [220.00 64.00 39.00x21.00] "grid 1"
    Container [424.00 60.00 208.00x29.00] content 50.00x29.00
      Text [428.00 64.00 42.00x21.00] content 42.00x21.00
        · text [428.00 64.00 42.00x21.00] "grid 2"
    Container [8.00 89.00 208.00x29.00] content 50.00x29.00
      Text [12.00 93.00 42.00x21.00] content 42.00x21.00
        · text [12.00 93.00 42.00x21.00] "grid 3"
    Container [216.00 89.00 208.00x29.00] content 50.00x29.00
      Text [220.00 93.00 42.00x21.00] content 42.00x21.00
        · text [220.00 93.00 42.00x21.00] "grid 4"
    Container [424.00 89.00 208.00x29.00] content 49.00x29.00
      Text [428.00 93.00 41.00x21.00] content 41.00x21.00
        · text [428.00 93.00 41.00x21.00] "grid 5"
    Container [8.00 118.00 208.00x29.00] content 50.00x29.00
      Text [12.00 122.00 42.00x21.00] content 42.00x21.00
        · text [12.00 122.00 42.00x21.00] "grid 6"
  Row [8.00 155.00 300.00x62.00] content 284.00x62.00
    Container [8.00 155.00 45.00x29.00] content 45.00x29.00
      Text [12.00 159.00 37.00x21.00] content 37.00x21.00
        · text [12.00 159.00 37.00x21.00] "tag 0"
    Container [57.00 155.00 41.00x29.00] content 41.00x29.00
      Text [61.00 159.00 33.00x21.00] content 33.00x21.00
        · text [61.00 159.00 33.00x21.00] "tag 1"
    Container [102.00 155.00 44.00x29.00] content 44.00x29.00
      Text [106.00 159.00 36.00x21.00] content 36.00x21.00
        · text [106.00 159.00 36.00x21.00] "tag 2"
    Container [150.00 155.00 45.00x29.00] content 45.00x29.00
      Text [154.00 159.00 37.00x21.00] content 37.00x21.00
        · text [154.00 159.00 37.00x21.00] "tag 3"
    Container [199.00 155.00 45.00x29.00] content 45.00x29.00
      Text [203.00 159.00 37.00x21.00] content 37.00x21.00
        · text [203.00 159.00 37.00x21.00] "tag 4"
    Container [248.00 155.00 44.00x29.00] content 44.00x29.00
      Text [252.00 159.00 36.00x21.00] content 36.00x21.00
        · text [252.00 159.00 36.00x21.00] "tag 5"
    Container [8.00 188.00 45.00x29.00] content 45.00x29.00
      Text [12.00 192.00 37.00x21.00] content 37.00x21.00
        · text [12.00 192.00 37.00x21.00] "tag 6"
    Container [57.00 188.00 44.00x29.00] content 44.00x29.00
      Text [61.00 192.00 36.00x21.00] content 36.00x21.00
        · text [61.00 192.00 36.00x21.00] "tag 7"
    Container [105.00 188.00 45.00x29.00] content 45.00x29.00
      Text [109.00 192.00 37.00x21.00] content 37.00x21.00
        · text [109.00 192.00 37.00x21.00] "tag 8"
  Container [8.00 225.00 624.00x80.00] content 340.00x51.00
    Text [293.00 255.00 55.00x21.00] content 55.00x21.00
      · text [293.00 255.00 55.00x21.00] "centred"
=== fitting
Column [0.00 0.00 640.00x480.00] content 640.00x480.00
  Text [0.00 0.00 640.00x30.00] content 201.00x30.00
    · text [0.00 0.00 640.00x30.00] "A pane that must fit"
  Container [0.00 30.00 640.00x429.00] content 640.00x429.00
    Scrollable [8.00 38.00 624.00x413.00] content 614.00x840.00
      · quad [625.35 40.00 3.30x201.09] clip [8.00 38.00 624.00x413.00]
      Column [8.00 38.00 614.00x840.00] content 614.00x840.00 clip [8.00 38.00 624.00x413.00]
        Text [8.00 38.00 614.00x21.00] content 39.00x21.00 clip [8.00 38.00 624.00x413.00]
          · text [8.00 38.00 614.00x21.00] "line 0" clip [8.00 38.00 624.00x413.00]
        Text [8.00 59.00 614.00x21.00] content 36.00x21.00 clip [8.00 38.00 624.00x413.00]
          · text [8.00 59.00 614.00x21.00] "line 1" clip [8.00 38.00 624.00x413.00]
        Text [8.00 80.00 614.00x21.00] content 39.00x21.00 clip [8.00 38.00 624.00x413.00]
          · text [8.00 80.00 614.00x21.00] "line 2" clip [8.00 38.00 624.00x413.00]
        Text [8.00 101.00 614.00x21.00] content 39.00x21.00 clip [8.00 38.00 624.00x413.00]
          · text [8.00 101.00 614.00x21.00] "line 3" clip [8.00 38.00 624.00x413.00]
        Text [8.00 122.00 614.00x21.00] content 39.00x21.00 clip [8.00 38.00 624.00x413.00]
          · text [8.00 122.00 614.00x21.00] "line 4" clip [8.00 38.00 624.00x413.00]
        Text [8.00 143.00 614.00x21.00] content 38.00x21.00 clip [8.00 38.00 624.00x413.00]
          · text [8.00 143.00 614.00x21.00] "line 5" clip [8.00 38.00 624.00x413.00]
        Text [8.00 164.00 614.00x21.00] content 39.00x21.00 clip [8.00 38.00 624.00x413.00]
          · text [8.00 164.00 614.00x21.00] "line 6" clip [8.00 38.00 624.00x413.00]
        Text [8.00 185.00 614.00x21.00] content 38.00x21.00 clip [8.00 38.00 624.00x413.00]
          · text [8.00 185.00 614.00x21.00] "line 7" clip [8.00 38.00 624.00x413.00]
        Text [8.00 206.00 614.00x21.00] content 39.00x21.00 clip [8.00 38.00 624.00x413.00]
          · text [8.00 206.00 614.00x21.00] "line 8" clip [8.00 38.00 624.00x413.00]
        Text [8.00 227.00 614.00x21.00] content 39.00x21.00 clip [8.00 38.00 624.00x413.00]
          · text [8.00 227.00 614.00x21.00] "line 9" clip [8.00 38.00 624.00x413.00]
        Text [8.00 248.00 614.00x21.00] content 45.00x21.00 clip [8.00 38.00 624.00x413.00]
          · text [8.00 248.00 614.00x21.00] "line 10" clip [8.00 38.00 624.00x413.00]
        Text [8.00 269.00 614.00x21.00] content 42.00x21.00 clip [8.00 38.00 624.00x413.00]
          · text [8.00 269.00 614.00x21.00] "line 11" clip [8.00 38.00 624.00x413.00]
        Text [8.00 290.00 614.00x21.00] content 45.00x21.00 clip [8.00 38.00 624.00x413.00]
          · text [8.00 290.00 614.00x21.00] "line 12" clip [8.00 38.00 624.00x413.00]
        Text [8.00 311.00 614.00x21.00] content 45.00x21.00 clip [8.00 38.00 624.00x413.00]
          · text [8.00 311.00 614.00x21.00] "line 13" clip [8.00 38.00 624.00x413.00]
        Text [8.00 332.00 614.00x21.00] content 45.00x21.00 clip [8.00 38.00 624.00x413.00]
          · text [8.00 332.00 614.00x21.00] "line 14" clip [8.00 38.00 624.00x413.00]
        Text [8.00 353.00 614.00x21.00] content 45.00x21.00 clip [8.00 38.00 624.00x413.00]
          · text [8.00 353.00 614.00x21.00] "line 15" clip [8.00 38.00 624.00x413.00]
        Text [8.00 374.00 614.00x21.00] content 45.00x21.00 clip [8.00 38.00 624.00x413.00]
          · text [8.00 374.00 614.00x21.00] "line 16" clip [8.00 38.00 624.00x413.00]
        Text [8.00 395.00 614.00x21.00] content 44.00x21.00 clip [8.00 38.00 624.00x413.00]
          · text [8.00 395.00 614.00x21.00] "line 17" clip [8.00 38.00 624.00x413.00]
        Text [8.00 416.00 614.00x21.00] content 45.00x21.00 clip [8.00 38.00 624.00x413.00]
          · text [8.00 416.00 614.00x21.00] "line 18" clip [8.00 38.00 624.00x413.00]
        Text [8.00 437.00 614.00x21.00] content 45.00x21.00 clip [8.00 38.00 624.00x413.00]
          · text [8.00 437.00 614.00x21.00] "line 19" clip [8.00 38.00 624.00x413.00]
        Text [8.00 458.00 614.00x21.00] content 48.00x21.00 clip [8.00 38.00 624.00x413.00]
          · text [8.00 458.00 614.00x21.00] "line 20" clip [8.00 38.00 624.00x413.00]
        Text [8.00 479.00 614.00x21.00] content 45.00x21.00 clip [8.00 38.00 624.00x413.00]
          · text [8.00 479.00 614.00x21.00] "line 21" clip [8.00 38.00 624.00x413.00]
        Text [8.00 500.00 614.00x21.00] content 48.00x21.00 clip [8.00 38.00 624.00x413.00]
          · text [8.00 500.00 614.00x21.00] "line 22" clip [8.00 38.00 624.00x413.00]
        Text [8.00 521.00 614.00x21.00] content 48.00x21.00 culled
        Text [8.00 542.00 614.00x21.00] content 48.00x21.00 culled
        Text [8.00 563.00 614.00x21.00] content 48.00x21.00 culled
        Text [8.00 584.00 614.00x21.00] content 48.00x21.00 culled
        Text [8.00 605.00 614.00x21.00] content 47.00x21.00 culled
        Text [8.00 626.00 614.00x21.00] content 48.00x21.00 culled
        Text [8.00 647.00 614.00x21.00] content 48.00x21.00 culled
        Text [8.00 668.00 614.00x21.00] content 48.00x21.00 culled
        Text [8.00 689.00 614.00x21.00] content 45.00x21.00 culled
        Text [8.00 710.00 614.00x21.00] content 48.00x21.00 culled
        Text [8.00 731.00 614.00x21.00] content 48.00x21.00 culled
        Text [8.00 752.00 614.00x21.00] content 49.00x21.00 culled
        Text [8.00 773.00 614.00x21.00] content 48.00x21.00 culled
        Text [8.00 794.00 614.00x21.00] content 48.00x21.00 culled
        Text [8.00 815.00 614.00x21.00] content 47.00x21.00 culled
        Text [8.00 836.00 614.00x21.00] content 48.00x21.00 culled
        Text [8.00 857.00 614.00x21.00] content 48.00x21.00 culled
  Text [0.00 459.00 640.00x21.00] content 43.00x21.00
    · text [0.00 459.00 640.00x21.00] "footer"
"####;
