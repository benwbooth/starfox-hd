//! MARIO scaled-text message lookup.
//!
//! `textpath` stores the low word of a `msg_*` record from MDATA.MC in
//! `dl_coltab`; `MDRAWLIS.MC` switches to the fixed `mariomsgs` bank and
//! prints that zero-terminated record. The HD renderer performs the same
//! lookup explicitly because its font atlas contains ASCII glyphs.

use std::borrow::Cow;

const END_SCORE_DIGIT_TAG: u16 = 0x4D00;
const END_SCORE_PERCENT_TAG: u16 = 0x4E00;
const END_SCORE_TOTAL_LABEL_TAG: u16 = 0x4F00;
const END_SCORE_AVERAGE_LABEL_TAG: u16 = 0x4F01;
const END_SCORE_STAGE_LABEL_TAG: u16 = 0x4F02;
const MESSAGE_TAG_MASK: u16 = 0xFF00;
const MESSAGE_VALUE_MASK: u16 = 0x00FF;

pub fn message_text(ptr: u16) -> Option<Cow<'static, str>> {
    // End-score digit compatibility tag (`sf_strat::endscore::MSG_DIGIT_TAG`).
    if ptr & MESSAGE_TAG_MASK == END_SCORE_DIGIT_TAG {
        return Some(Cow::Owned(((ptr & MESSAGE_VALUE_MASK) >> 1).to_string()));
    }
    if ptr & MESSAGE_TAG_MASK == END_SCORE_PERCENT_TAG {
        return Some(Cow::Owned(format!("{:>3}%", ptr & MESSAGE_VALUE_MASK)));
    }
    match ptr {
        END_SCORE_TOTAL_LABEL_TAG => return Some(Cow::Borrowed("TOTAL SCORE")),
        END_SCORE_AVERAGE_LABEL_TAG => return Some(Cow::Borrowed("AVERAGE SCORE")),
        END_SCORE_STAGE_LABEL_TAG => return Some(Cow::Borrowed("STAGE")),
        _ => {}
    }

    Some(Cow::Borrowed(match ptr {
        0xC8DA => "STAR FOX",
        0xC8E3 => "NINTENDO",
        0xC8EC => "PRESENTED",
        0xC8F6 => "PRESENTS",
        0xC8FF => "ASSISTED",
        0xC90C => "PROGRAMMED",
        0xC917 => "BY",
        0xC91A => "ARGONAUT SOFTWARE",
        0xC92C => "EXECUTIVE PRODUCER",
        0xC93F => "HIROSHI YAMAUCHI",
        0xC950 => "PRODUCER",
        0xC959 => "SHIGERU MIYAMOTO",
        0xC96A => "DIRECTOR",
        0xC973 => "KATSUYA EGUCHI",
        0xC982 => "ASSISTANT DIRECTOR",
        0xC995 => "YOICHI YAMADA",
        0xC9A3 => "DYLAN CUTHBERT",
        0xC9B2 => "GILES GODDARD",
        0xC9C0 => "KRISTER WOMBELL",
        0xC9D0 => "3D SYSTEM",
        0xC9DA => "PETE WARNES",
        0xC9E6 => "CARL GRAHAM",
        0xC9F2 => "GRAPHIC DESIGNER",
        0xCA03 => "TAKAYA IMAMURA",
        0xCA12 => "SHAPE DESIGNER",
        0xCA21 => "TSUYOSHI WATANABE",
        0xCA33 => "KOJI KONDO",
        0xCA3E => "HAJIME HIRASAWA",
        0xCA4E => "SUPER FX STAFF",
        0xCA5D => "BEN CHEESE",
        0xCA68 => "SATOSHI NISHIUMI",
        0xCA79 => "HIRONOBU KAKUI",
        0xCA88 => "SHIGEKI YAMASHIRO",
        0xCA9A => "YASUHIRO KAWAGUCHI",
        0xCAB7 => "JEZ SAN",
        0xCABF => "KEIZO KATO",
        0xCACA => "SOUND EFFECTS",
        0xCAD8 => "MUSIC COMPOSER",
        0xCAE7 => "YASUNARI NISHIDA",
        0xCAF8 => "IAN CROWTHER",
        0xCB05 => "DAN OWSEN",
        0xCB0F => "TONY HARMAN",
        0xCB1B => "MASATO KIMURA",
        0xCB29 => "TAKAO SHIMIZU",
        0xCB37 => "HAJIME YAJIMA",
        0xCB45 => "KENJI YAMAMOTO",
        0xCB5C => "ENGLISH SUPPORT",
        0xCB6C => "JAPANESE SUPPORT",
        0xCB7D => "SOFTWARE SUPPORT",
        0xCB8E => "RICHARD CLUCAS",
        0xCB9D => "JON DEAN",
        _ => return None,
    }))
}
