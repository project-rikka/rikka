use crate::corpus::Case;

#[derive(Clone, Copy, Debug)]
pub struct ScriptCorpus {
    pub group: &'static str,
    pub fonts: &'static [&'static str],
    pub texts: &'static [&'static str],
}

pub const SYLLABIC_CORPORA: [ScriptCorpus; 19] = [
    ScriptCorpus {
        group: "devanagari",
        fonts: &["NotoSansDevanagari.ttf", "NotoSerifDevanagari.ttf"],
        texts: &[
            "नमस्ते दुनिया, हिन्दी भाषा सुंदर है।",
            "क्षत्रिय ज्ञान श्री द्व द्ध ह्न ह्म ट्ट ड्ड ङ्क्ष",
            "र्क र्कि र्की कर्म धर्म्य पूर्व आर्द्र र्‍य",
            "कि क्ति स्त्रि ष्ट्रि त्र्य क्ष्मि",
            "क्\u{200D}ष क्\u{200C}ष र्\u{200D}य र\u{200D}्य क\u{200D}्ष",
            "\u{93F}क क\u{94D}\u{94D} अ\u{93E} अ\u{945} र\u{94D}इ \u{902}\u{94D}",
            "ॐ ऽ । ॥ ०१२३४५ ₹१०",
            "क़ ख़ ग़ क\u{93C} ड\u{93C}\u{93C} य़ ऱ",
            "हँसी अँधेरा कं कः कॅ कॉ",
            "द्ध्र्य ज़्ज़ क्\u{94D}",
        ],
    },
    ScriptCorpus {
        group: "bengali",
        fonts: &["NotoSansBengali.ttf", "NotoSerifBengali.ttf"],
        texts: &[
            "আমি বাংলায় গান গাই।",
            "ক্ষ জ্ঞ ন্ত্র র্ক কি কে কো কৌ",
            "র‍্য ব্য ব্য্র য় য\u{9BC} ড\u{9BC}",
            "ক\u{9C7}\u{9BE} ক\u{9C7}\u{9D7} \u{9CB} অ\u{9BE} ঋ\u{9C3}",
            "ৎ ক্\u{200D} ক্\u{200C}ষ র্‌য",
        ],
    },
    ScriptCorpus {
        group: "gurmukhi",
        fonts: &["NotoSansGurmukhi.ttf", "NotoSerifGurmukhi.ttf"],
        texts: &[
            "ਪੰਜਾਬੀ ਭਾਸ਼ਾ ਗੁਰਮੁਖੀ",
            "ਕ੍ਰ ਸ੍ਵ ਕਿ ਕੀ ਖ਼ ਪ੍ਰਿ ਸ੍ਹ",
            "ਅ\u{A3E} ੲ\u{A3F} ੳ\u{A41} \u{A3F}ਕ ਕ\u{A71}ਕ",
        ],
    },
    ScriptCorpus {
        group: "gujarati",
        fonts: &["NotoSansGujarati.ttf", "NotoSerifGujarati.ttf"],
        texts: &[
            "ગુજરાતી ભાષા સુંદર છે",
            "ક્ષ જ્ઞ ર્ક કિ શ્રી દ્ર ત્ર્ય",
            "અ\u{ABE} \u{AC5}\u{ABE} \u{ABF}ક",
        ],
    },
    ScriptCorpus {
        group: "oriya",
        fonts: &["NotoSansOriya.ttf", "NotoSerifOriya.ttf"],
        texts: &[
            "ଓଡ଼ିଆ ଭାଷା",
            "କ୍ଷ ର୍କ କି କୈ କୌ କ୍ୟ ସ୍ତ୍ର",
            "ଅ\u{B3E} କ\u{B47}\u{B3E} କ\u{B47}\u{B56} \u{B48}",
        ],
    },
    ScriptCorpus {
        group: "tamil",
        fonts: &["NotoSansTamil.ttf", "NotoSerifTamil.ttf"],
        texts: &[
            "தமிழ் மொழி இனிமையானது",
            "க்ஷ ஸ்ரீ கொ கோ கௌ க்க ஷ்ரீ",
            "க\u{BC6}\u{BBE} க\u{BC7}\u{BBE} க\u{BC6}\u{BD7} \u{BCA} அ\u{BC2}",
            "ஶ்ரீ க்‌ஷ",
        ],
    },
    ScriptCorpus {
        group: "telugu",
        fonts: &["NotoSansTelugu.ttf", "NotoSerifTelugu.ttf"],
        texts: &[
            "తెలుగు భాష చాలా అందమైనది",
            "క్ష ర్క క్రి క్కు శ్రీ స్త్రీ",
            "ర్\u{200D}క క\u{C46}\u{C56} \u{C3F}\u{C55} ఒ\u{C55}",
        ],
    },
    ScriptCorpus {
        group: "kannada",
        fonts: &[
            "NotoSansKannada.ttf",
            "NotoSerifKannada.ttf",
            "NotoSansKannada-UI-VF.ttf",
        ],
        texts: &[
            "ಕನ್ನಡ ಭಾಷೆ ಸುಂದರವಾಗಿದೆ",
            "ಕ್ಷ ರ್ಕ ಕ್ರಿ ಶ್ರೀ ಕೊ ಕೋ ಕ್ಕೈ",
            "ರ್\u{200D}ಕ ರ\u{200D}್ಕ ಕ\u{CC6}\u{CC2}\u{CD5} ಉ\u{CBE}",
        ],
    },
    ScriptCorpus {
        group: "malayalam",
        fonts: &["NotoSansMalayalam.ttf", "NotoSerifMalayalam.ttf"],
        texts: &[
            "മലയാളം ഭാഷ മനോഹരമാണ്",
            "ക്ഷ ന്റ ന്ത ക്ക കൊ കോ കൗ സ്ത്ര",
            "ൻ ർ ൽ ൾ ൺ ൿ ന്‍ ര്‍",
            "ൎക്ക ൎത്ത ക\u{D46}\u{D3E} \u{D4A} ഇ\u{D57}",
            "പ്ര ക്ര ത്ര ഗ്ര്യ",
        ],
    },
    ScriptCorpus {
        group: "sinhala",
        fonts: &["NotoSansSinhala.ttf", "NotoSerifSinhala.ttf"],
        texts: &[
            "සිංහල භාෂාව ලස්සනයි",
            "ක්\u{200D}ෂ ශ්\u{200D}රී ක්\u{200D}ය ර්\u{200D}ක කො කෝ කෞ",
            "අ\u{DCF} එ\u{DCA} \u{DD9}ක ක\u{DD9}\u{DCF}\u{DCA}",
        ],
    },
    ScriptCorpus {
        group: "khmer",
        fonts: &["NotoSansKhmer.ttf", "NotoSerifKhmer.ttf"],
        texts: &[
            "ភាសាខ្មែរ ជាភាសាជាតិ",
            "ក្រ ស្ត្រ ក្ក្ក កើ កៀ កោ ង្ក្រ",
            "ក្\u{179A}ុំ ស៊ី ប៉ុ ស្រ្តី \u{17C1}ក ្ក",
            "ក\u{17C1}\u{17B8} ក៏ ក៌ ក៍ ១២៣",
        ],
    },
    ScriptCorpus {
        group: "myanmar",
        fonts: &["NotoSansMyanmar.ttf", "NotoSerifMyanmar.ttf"],
        texts: &[
            "မြန်မာဘာသာ စကား",
            "သင်္ဘော ကြ ကျွ ကြွ ကို ကော် ကောင်း",
            "\u{103C}က က\u{103A}\u{103A} \u{1031}ေ ဿ ၊ ။",
            "ၵႂ်ႈ ႁဵဝ်း ꩡ ကၞ",
        ],
    },
    ScriptCorpus {
        group: "tibetan",
        fonts: &["NotoSerifTibetan.ttf"],
        texts: &[
            "བོད་ཡིག་ཀྱི་ཡི་གེ།",
            "བསྒྲུབས་ སྐྱེས་ ཧཱུྃ་ ཨོཾ་མ་ཎི་པདྨེ་ཧཱུྃ",
            "ཀྵ ཀ྄ ཀཱི ཀྀ ༀ ༡༢༣ \u{F71}ཀ",
        ],
    },
    ScriptCorpus {
        group: "javanese",
        fonts: &["NotoSansJavanese.ttf"],
        texts: &[
            "ꦧꦱꦗꦮ ꦲꦏ꧀ꦱꦫ",
            "ꦏꦿ ꦏꦾ ꦏꦽ ꦏ꧀ꦭ ꦏꦺꦴ ꦏꦼ ꦏꦁ ꦏꦃ",
            "ꦄ ꦲꦶ ꦁꦏ \u{A9BF}ꦏ ꧈ ꧉",
        ],
    },
    ScriptCorpus {
        group: "balinese",
        fonts: &["NotoSansBalinese.ttf", "NotoSerifBalinese-Regular.otf"],
        texts: &[
            "ᬅᬓ᭄ᬱᬭ ᬩᬮᬶ",
            "ᬓ᭄ᬭ ᬓ᭄ᬬ ᬓᭀ ᬓᬿ ᬓᭀ ᬓᬂ ᬓᬄ",
            "\u{1B3E}ᬓ ᬓ\u{1B3E}\u{1B35} ᭐᭑᭒",
        ],
    },
    ScriptCorpus {
        group: "sundanese",
        fonts: &["NotoSansSundanese.ttf"],
        texts: &["ᮘᮞ ᮞᮥᮔ᮪ᮓ", "ᮊᮢ ᮊᮡ ᮊᮦ ᮊᮨ ᮊᮀ ᮊ᮪"],
    },
    ScriptCorpus {
        group: "nko",
        fonts: &["NotoSansNKo-Regular.otf", "NotoSansNKoUnjoined.ttf"],
        texts: &["ߒߞߏ ߞߊ߲ߜߍ߫ ߓߊ߯ߙߊ", "ߞ߫߭ ߊ߲߬ \u{7EB}ߓ ߸ ߹"],
    },
    ScriptCorpus {
        group: "mongolian",
        fonts: &["NotoSansMongolian-Regular.otf"],
        texts: &[
            "ᠮᠣᠩᠭᠣᠯ ᠪᠢᠴᠢᠭ",
            "ᠭ\u{180B}ᠠ ᠨ\u{180C}ᠠ ᠠ\u{180E}ᠠ ᠠ\u{180F}",
            "᠀\u{180B} ᢀᢅ ᠰ\u{200D} \u{200D}ᠰ",
        ],
    },
    ScriptCorpus {
        group: "hieroglyphs",
        fonts: &["NotoSansEgyptianHieroglyphs-Regular.otf"],
        texts: &["𓀀𓁐𓂀 𓀀\u{13430}𓁐 \u{13437}𓀀\u{13438}"],
    },
];

pub const USE_SAMPLES: &[(&str, &[&str])] = &[
    (
        "NotoSansBatak-Regular.otf",
        &["ᯘᯮᯒᯖ᯲ ᯅᯖᯂ᯲ ᯀᯩ ᯂᯪ ᯖ᯲ᯒ", "ᯂ᯳ ᯅᯮ \u{1BE7}ᯂ"],
    ),
    (
        "NotoSansBuginese-Regular.otf",
        &["ᨅᨔ ᨕᨘᨁᨗ ᨀᨚ ᨒᨛ", "ᨀ\u{1A19}\u{1A17}"],
    ),
    (
        "NotoSansChakma-Regular.otf",
        &["𑄌𑄋𑄴𑄟 𑄇𑄴𑄇 𑄇𑄳𑄢 𑄇𑄮 𑄇𑄨𑄁", "𑄇\u{11127}\u{11134} \u{11133}𑄇"],
    ),
    ("NotoSansCham.ttf", &["ꨌꩌ ꨆꨴ ꨀꨯ ꨆꨶꨮ ꨆꩃ", "ꨆ\u{AA2F}\u{AA35}"]),
    (
        "NotoSansLepcha-Regular.otf",
        &["ᰛᰩᰵ ᰀᰦ ᰀᰤ ᰀᰭ ᰀᰶ", "ᰀ\u{1C27}\u{1C36}"],
    ),
    (
        "NotoSansLimbu-Regular.otf",
        &["ᤕᤠᤰᤌᤢᤱ ᤁᤩ ᤁᤪ ᤁ᤺ᤰ", "ᤁ\u{1932}\u{193B}"],
    ),
    (
        "NotoSansNewa-Regular.otf",
        &["𑐣𑐾𑐥𑐵𑐮 𑐎𑑂𑐫 𑐎𑑂𑐬𑐶 𑐬𑑂𑐎 𑐎𑐸", "\u{11446}𑐎 𑐎\u{11442}\u{11442}"],
    ),
    (
        "NotoSansSharada-Regular.otf",
        &["𑆯𑆳𑆫𑆢𑆳 𑆑𑇀𑆰 𑆑𑆴 𑆫𑇀𑆑", "𑆑\u{111C0}\u{111C0}"],
    ),
    (
        "NotoSansSiddham-Regular.otf",
        &["𑖭𑖰𑖟𑖿𑖠𑖽 𑖎𑖿𑖬 𑖎𑖸 𑖨𑖿𑖎", "𑖎\u{115BF}\u{115BF} \u{115B8}𑖎"],
    ),
    (
        "NotoSansTakri-Regular.otf",
        &["𑚔𑚭𑚊𑚤𑚯 𑚊𑚶𑚤 𑚊𑚲 𑚀𑚭", "𑚀\u{116B4} 𑚆\u{116B2}"],
    ),
    (
        "NotoSansTirhuta-Regular.otf",
        &["𑒞𑒱𑒩𑒯𑒳𑒞 𑒏𑓂𑒭 𑒏𑒱 𑒩𑓂𑒏", "𑒁\u{114B0} 𑒪\u{114B5}"],
    ),
    (
        "NotoSansModi-Regular.otf",
        &["𑘦𑘻𑘚𑘲 𑘎𑘿𑘬 𑘎𑘱 𑘨𑘿𑘎", "𑘀\u{11639} 𑘁\u{1163A}"],
    ),
    (
        "NotoSansKhojki-Regular.otf",
        &["𑈀𑈁𑈂 𑈈𑈶𑈥 𑈈𑈬 𑈈𑈵𑈩", "𑈀\u{1122C}\u{11231} 𑈬\u{11230}"],
    ),
    (
        "NotoSansKaithi-Regular.otf",
        &["𑂍𑂰𑂟𑂲 𑂍𑂹𑂩 𑂍𑂱 𑂩𑂹𑂍", "\u{110BD}𑂍 𑂍\u{110B9}\u{110B9}"],
    ),
    (
        "NotoSansBrahmi-Regular.otf",
        &["𑀩𑁆𑀭𑀸𑀳𑁆𑀫𑀻 𑀓𑁆𑀱 𑀓𑀺 𑀭𑁆𑀓", "𑀅\u{11038} 𑀏\u{11042} 𑁒𑁓"],
    ),
    (
        "NotoSansGrantha-Regular.otf",
        &["𑌗𑍍𑌰𑌨𑍍𑌥 𑌕𑍍𑌷 𑌕𑍇 𑌕𑍋 𑌕𑍌", "𑌕\u{11347}\u{1133E} \u{11348}𑌕"],
    ),
    (
        "NotoSerifGrantha-Regular.otf",
        &["𑌗𑍍𑌰𑌨𑍍𑌥 𑌕𑍍𑌷 𑌕𑍇 𑌕𑍋", "𑌕\u{11303}\u{11303}"],
    ),
    (
        "NotoSansTaiTham.ttf",
        &["ᨠᩮᩬᩣ ᨴ᩠ᨶ ᨠ᩠ᨿ ᨠᩯ ᨠᩴ", "ᨠ\u{1A60}ᨶ \u{1A6E}ᨠ ᨠ\u{1A75}\u{1A7A}"],
    ),
    (
        "NotoSansMeeteiMayek.ttf",
        &["ꯃꯤꯇꯩ ꯃꯌꯦꯛ ꯀꯨ ꯀ꯭ꯌ", "ꯀ\u{ABED}\u{ABED}"],
    ),
    (
        "NotoSansSaurashtra-Regular.otf",
        &["ꢱꣃꢬ꣄ ꢒ꣄ꢥ ꢒꢶ ꢒꣀ", "ꢒ\u{A8C4}\u{A8C4}"],
    ),
    (
        "NotoSansSylotiNagri-Regular.otf",
        &["ꠍꠤꠟꠐꠤ ꠇꠣ ꠇ꠆ꠞ ꠇꠦ", "ꠇ\u{A806}\u{A806}"],
    ),
    (
        "NotoSansTagalog-Regular.otf",
        &["ᜊᜌ᜔ᜊᜌᜒᜈ᜔ ᜃᜓ ᜃ᜕", "ᜃ\u{1712}\u{1713}"],
    ),
    (
        "NotoSansHanunoo-Regular.otf",
        &["ᜱᜨᜳ ᜣᜲ ᜣ᜴", "ᜣ\u{1732}\u{1733}"],
    ),
    ("NotoSansBuhid-Regular.otf", &["ᝊᝓᝑᝒ ᝃᝒ", "ᝃ\u{1752}\u{1753}"]),
    (
        "NotoSansTagbanwa-Regular.otf",
        &["ᝦᝮ ᝣᝲ", "ᝣ\u{1772}\u{1773}"],
    ),
    (
        "NotoSansRejang-Regular.otf",
        &["ꤰꥈꤴ ꤰꥇ ꤰ꥓", "ꤰ\u{A947}\u{A947}"],
    ),
    (
        "NotoSansTaiLe-Regular.otf",
        &["ᥖᥭᥰ ᥘᥫᥴ ᥐᥨ", "ᥐ\u{1970}\u{1970}"],
    ),
    (
        "NotoSansTaiViet-Regular.otf",
        &["ꪁꪱ ꪹꪁ ꪀꪲ ꪀꪷ", "ꪀ\u{AAB2}\u{AAB4}"],
    ),
    ("NotoSansKayahLi.ttf", &["ꤊꤢ꤭ ꤊꤦ ꤊꤪ", "ꤊ\u{A92B}\u{A92C}"]),
    (
        "NotoSansAdlam.ttf",
        &["𞤀𞤣𞤤𞤢𞤥 𞤆𞤵𞤤𞤢𞤪 𞤀𞥄𞤣𞤢𞥅𞤥", "𞤀\u{1E944}\u{1E94A} \u{1E946}𞤀"],
    ),
    ("NotoSansAdlamUnjoined.ttf", &["𞤀𞤣𞤤𞤢𞤥 𞤆𞤵𞤤𞤢𞤪", "𞤀\u{1E944}"]),
    (
        "NotoSansWancho-Regular.otf",
        &["𞋁𞋠𞋒 𞋀𞋬 𞋀𞋭", "𞋀\u{1E2EC}\u{1E2ED}"],
    ),
    (
        "NotoSansNagMundari.ttf",
        &["𞓐𞓢𞓚 𞓐𞓬 𞓐𞓮", "𞓐\u{1E4EC}\u{1E4EE}"],
    ),
    ("NotoSansKawi.ttf", &["𑼒𑽂𑼦 𑼒𑽂𑼬 𑼒𑼴 𑼒𑼁", "𑼒\u{11F42}\u{11F42}"]),
    (
        "NotoSansGunjalaGondi.ttf",
        &["𑵱𑶗𑵳 𑵱𑶊 𑵱𑶗𑶂", "𑵱\u{11D97}\u{11D97}"],
    ),
    (
        "NotoSansMasaramGondi-Regular.otf",
        &["𑴌𑵅𑴪 𑴌𑴱 𑴌𑵄𑴦", "𑴌\u{11D44}\u{11D45}"],
    ),
    (
        "NotoSerifDogra-Regular.otf",
        &["𑠌𑠹𑠪 𑠌𑠬 𑠌𑠹𑠦", "𑠌\u{11839}\u{11839}"],
    ),
    (
        "NotoSansNandinagari-Regular.otf",
        &["𑦮𑧠𑦱 𑦮𑧑 𑦮𑧠𑧁", "𑦮\u{119E0}\u{119E0}"],
    ),
    (
        "NotoSansSoyombo-Regular.otf",
        &["𑩜𑪙𑩫 𑩜𑩑 𑩜𑪙𑩻", "𑩜\u{11A99}\u{11A99}"],
    ),
    (
        "NotoSansZanabazarSquare-Regular.otf",
        &["𑨀𑨴 𑨋𑨴𑨪 𑨋𑨁", "𑨋\u{11A34}\u{11A34}"],
    ),
    (
        "NotoSansBhaiksuki-Regular.otf",
        &["𑰎𑰿𑰨 𑰎𑰯 𑰎𑰿𑰦", "𑰎\u{11C3F}\u{11C3F}"],
    ),
    (
        "NotoSansMarchen-Regular.otf",
        &["𑲉𑲲 𑲉𑲩 𑲉𑲱", "𑲉\u{11CB2}\u{11CB2}"],
    ),
    (
        "NotoSerifDivesAkuru-Regular.otf",
        &["𑤌𑤽𑤪 𑤌𑤰 𑤌𑤾𑤪", "𑤌\u{1193D}\u{1193D}"],
    ),
    (
        "NotoSerifAhom-Regular.otf",
        &["𑜒𑜫𑜀 𑜒𑜡 𑜒𑜫𑜊", "𑜒\u{1172B}\u{1172B}"],
    ),
    ("NotoSansTangsa.ttf", &["𖩵𖩿 𖩰𖪃 𖩰𖪉", "𖩰\u{16AC0}"]),
    (
        "NotoSerifToto.ttf",
        &["𞊐𞊣 𞊐𞊡 𞊐\u{1E2AE}", "𞊐\u{1E2AE}\u{1E2AE}"],
    ),
    ("NotoSansSunuwar-Regular.otf", &["𑯀𑯁𑯂 𑯃𑯄", "𑯀\u{11BE0}"]),
    ("NotoSerifTodhri-Regular.otf", &["𐗀𐗁𐗂 𐗃𐗄", "𐗀\u{105C9}"]),
    (
        "NotoSansHanifiRohingya.ttf",
        &["𐴀𐴁𐴂𐴃 𐴄𐴅𐴆 𐴀𐴤 𐴀𐴧", "𐴀\u{10D24}\u{10D27} 𐴰𐴱"],
    ),
    (
        "NotoSansManichaean-Regular.otf",
        &["𐫀𐫁𐫂𐫃 𐫄𐫅", "𐫀\u{10AE5}\u{10AE6}"],
    ),
    ("NotoSansPsalterPahlavi-Regular.otf", &["𐮀𐮁𐮂𐮃 𐮄𐮅", "𐮀𐮙"]),
    (
        "NotoSansSogdian-Regular.otf",
        &["𐼰𐼱𐼲𐼳 𐼴𐼵", "𐼰\u{10F46}\u{10F48}"],
    ),
    ("NotoSansPhagsPa-Regular.otf", &["ꡀꡁꡂꡃ ꡄꡅ ꡀ\u{200D}", "ꡀ᠂"]),
    (
        "NotoSansMandaic-Regular.otf",
        &["ࡀࡁࡂ ࡃࡄ", "ࡀ\u{859}\u{85A}"],
    ),
    ("NotoSansChorasmian-Regular.otf", &["𐾰𐾱𐾲 𐾳𐾴", "𐾰𐾱"]),
    (
        "NotoSerifOldUyghur-Regular.otf",
        &["𐽰𐽱𐽲 𐽳𐽴", "𐽰\u{10F82}\u{10F83}"],
    ),
    ("NotoSansMiao-Regular.otf", &["𖼀𖽑𖾏 𖼀𖽒𖾐", "𖼀\u{16F8F}"]),
    (
        "NotoSansPahawhHmong-Regular.otf",
        &["𖬀𖬰 𖬁𖬱", "𖬀\u{16B30}\u{16B31}"],
    ),
    (
        "NotoSansDuployan-Regular.otf",
        &["𛰀𛰁𛰂 𛰃\u{1BC9D}", "𛰀\u{1BC9E}"],
    ),
    (
        "NotoSansKharoshthi-Regular.otf",
        &["𐨐𐨿𐨪 𐨐𐨁 𐨐𐨌", "𐨐\u{10A38}\u{10A0D}"],
    ),
    ("NotoSansMultani-Regular.otf", &["𑊄𑊅𑊆 𑊇𑊈", "𑊄𑊅"]),
    ("NotoSansMahajani-Regular.otf", &["𑅐𑅑𑅒 𑅳𑅓", "𑅐\u{11173}"]),
    ("NotoSansKhudawadi-Regular.otf", &["𑊰𑋠 𑊺𑋪𑋥", "𑊰\u{112E5}"]),
    (
        "NotoSerifMakasar-Regular.otf",
        &["𑻠𑻳 𑻡𑻴", "𑻠\u{11EF3}\u{11EF4}"],
    ),
    ("NotoSansMedefaidrin.ttf", &["𖹀𖹁𖹂 𖹃𖹄", "𖹀𖹁"]),
    ("NotoSansElymaic-Regular.otf", &["𐿠𐿡𐿢 𐿣𐿤", "𐿠𐿡"]),
    ("NotoSansOldSogdian-Regular.otf", &["𐼀𐼁𐼂 𐼃𐼄", "𐼀𐼁"]),
    (
        "NotoSerifKhitanSmallScript-Regular.otf",
        &["𘬀𘬁𘬂 𖿤𘬃", "𘬀𖿤"],
    ),
    (
        "NotoSansTifinagh-Regular.otf",
        &["ⵜⴰⵎⴰⵣⵉⵖⵜ ⵜ⵿ⵜ", "ⵜ\u{2D7F}\u{2D7F}"],
    ),
    ("NotoSansCyproMinoan-Regular.otf", &["𒾐𒾑𒾒", "𒾐𒾑"]),
    ("NotoSerifYezidi.ttf", &["𐺀𐺁𐺂 𐺃\u{10EAB}", "𐺀\u{10EAC}"]),
    ("NotoSansVithkuqi.ttf", &["𐕰𐕱𐕲 𐖗𐖘", "𐕰𐕱"]),
    ("NotoSerifNPHmong.ttf", &["𞄀𞄁𞄂 𞄃\u{1E130}", "𞄀\u{1E131}"]),
    ("NotoSansNewTaiLue.ttf", &["ᦀᦰᦂ ᦵᦂ ᧁ", "ᦂ\u{19C1}"]),
];

pub(crate) fn syllabic_cases(cases: &mut Vec<Case>) {
    for corpus in &SYLLABIC_CORPORA {
        for font in corpus.fonts {
            for text in corpus.texts {
                cases.push(Case::new(corpus.group, font, text));
                cases.push(Case::new(corpus.group, font, text).bot_eot());
            }
        }
    }
    for (font, texts) in USE_SAMPLES {
        for text in *texts {
            cases.push(Case::new("use", font, text));
            cases.push(Case::new("use", font, text).bot_eot());
        }
    }
}
