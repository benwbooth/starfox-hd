//! Wingman message state machine + English message data.
//!
//! C oracle: `src/game/strings.c` (STRINGS.ASM -> C conversion) and
//! `src/game/messages_data.c/h` (generated from
//! reference/ultrastarfox/SF/MSG/ENGLISH.INC).
//!
//! The C globals `g_whichfriend` / `g_friends_msg` / `g_msg_count1` /
//! `g_msg_count2` / `g_friends_sound` / `g_friends_meter` (game_vars.c)
//! live in [`Strings`]. Friend HP: `g_bunny_hp`/`g_falcon_hp`/`g_frog_hp`
//! are canonical in [`crate::vars::GameVars`] (the map VM's friend-alive
//! callbacks read them); the shell mirrors them into this struct at the top
//! of every tick (see shell.rs). `g_fox_hp`/`g_pepper_hp`/`g_andross_hp`
//! are not read by any ported system besides strings, so they live here
//! with the game_vars.c:461-466 default of 3.

use crate::shell::SoundCmd;

// C FRIEND_* (src/variables.h:250-256).
pub const FRIEND_FOX: u8 = 0;
pub const FRIEND_RABBIT: u8 = 1;
pub const FRIEND_FALCON: u8 = 2;
pub const FRIEND_FROG: u8 = 3;
pub const FRIEND_PEPPER: u8 = 4;
pub const FRIEND_ANDROSS: u8 = 5;
pub const FRIEND_ANYONE: u8 = 6;

/// C `STRINGS_FRIEND_VARIANT3_BIT` (src/game/messages_data.h).
pub const FRIEND_VARIANT3_BIT: u8 = 0x80;

// C STRINGS_SOUND_CLASS_* (src/game/messages_data.h).
pub const SOUND_CLASS_HELP: u8 = 0;
pub const SOUND_CLASS_DOWN: u8 = 1;
pub const SOUND_CLASS_OTHER: u8 = 2;

/// C `STRINGS_MESSAGE_ID_MIN`/`MAX` (src/game/messages_data.h).
pub const MESSAGE_ID_MIN: u8 = 1;
pub const MESSAGE_ID_MAX: u8 = 142;

// C CONTINUE.ASM constants (src/game/strings.c:26-27).
const OPENING_FRAMES: u8 = 5;
const MSG_CLOSE_SFX: u8 = 0x64;

/// C `s_face_sounds` (src/game/strings.c:29).
const FACE_SOUNDS: [u8; 24] = [
    0x60, 0x60, 0x60, 0x60, // fox
    0x7C, 0x7D, 0x62, 0x62, // rabbit
    0x7A, 0x7B, 0x61, 0x61, // falcon
    0x7E, 0x7F, 0x63, 0x63, // frog
    0x5F, 0x5F, 0x5F, 0x5F, // pepper
    0x8C, 0x8C, 0x8C, 0x8C, // andross
];

/// One message record: (whichfriend, sound_class, text). C
/// `StringsMessageData` (src/game/messages_data.h). Index 0 is the C
/// `[0] = { FRIEND_FOX, STRINGS_SOUND_CLASS_OTHER, NULL }` sentinel.
type MessageData = (u8, u8, Option<&'static str>);

/// C `s_english_messages_data` (src/game/messages_data.c:7-151), full port.
const ENGLISH_MESSAGES: [MessageData; MESSAGE_ID_MAX as usize + 1] = [
    (FRIEND_FOX, SOUND_CLASS_OTHER, None),
    (FRIEND_FOX, 2, Some("all ships check in!")),
    (FRIEND_FOX, 2, Some("we did it!")),
    (FRIEND_FOX, 2, Some("slippy slippy##")),
    (FRIEND_FOX, 2, Some("go back to the base!")),
    (FRIEND_FALCON, 2, Some("ready, fox!")),
    (FRIEND_FALCON, 2, Some("i'm with you, fox!")),
    (FRIEND_FALCON, 0, Some("bogey on my six!")),
    (FRIEND_FALCON, 2, Some("mind your own business, fox!")),
    (FRIEND_FALCON, 1, Some("they got me! i'm gone!!")),
    (FRIEND_FALCON, 2, Some("i'll blast 'em all!")),
    (FRIEND_FALCON, 2, Some("there's more on the way!")),
    (FRIEND_FALCON, 2, Some("you can run, but you can't hide!")),
    (FRIEND_FALCON, 2, Some("this one's mine!")),
    (FRIEND_FALCON, 0, Some("watch it, fox!")),
    (FRIEND_FALCON, 2, Some("i'm hit!!")),
    (FRIEND_FALCON, 2, Some("here they come! ")),
    (FRIEND_FALCON | 0x80, 2, Some("make the neck and tail shorter, fox!!")),
    (FRIEND_FALCON | 0x80, 2, Some("shoot down its arms to hit its body!")),
    (FRIEND_FALCON, 2, Some("slow it down, fox!")),
    (FRIEND_FALCON, 2, Some("roll, baby! rock 'n roll!")),
    (FRIEND_FALCON, 2, Some("my ship's messed up###")),
    (FRIEND_FALCON, 2, Some("hey! that one was mine!")),
    (FRIEND_FALCON, 2, Some("bogies, i'm comin' through!")),
    (FRIEND_FALCON, 2, Some("i'll cover your tail!")),
    (FRIEND_RABBIT, 2, Some("yeah - let's go!")),
    (FRIEND_RABBIT, 2, Some("i'm behind you, fox!")),
    (FRIEND_RABBIT, 0, Some("get lost, you fiend!")),
    (FRIEND_RABBIT, 2, Some("yeah! thanks!")),
    (FRIEND_RABBIT, 1, Some("aargh!! i'm a goner!")),
    (FRIEND_RABBIT, 2, Some("let's smash 'em!")),
    (FRIEND_RABBIT, 2, Some("i got one! i got one!!")),
    (FRIEND_RABBIT, 2, Some("take this, enemy brute!")),
    (FRIEND_RABBIT, 2, Some("you're not getting away!")),
    (FRIEND_RABBIT, 0, Some("watch your aim, fox!")),
    (FRIEND_RABBIT, 2, Some("ouch! ouch!")),
    (FRIEND_RABBIT, 2, Some("incoming enemy craft! ")),
    (FRIEND_RABBIT | 0x80, 2, Some("please make the neck and tail shorter, fox!")),
    (FRIEND_RABBIT | 0x80, 2, Some("shoot down its arms to hit its body!")),
    (FRIEND_RABBIT, 2, Some("retros!  fire retros!")),
    (FRIEND_RABBIT, 2, Some("roll over! shake it off!")),
    (FRIEND_RABBIT, 2, Some("pick me up on your way back!")),
    (FRIEND_RABBIT, 2, Some("hey! he was mine!")),
    (FRIEND_RABBIT, 2, Some("out of my way!")),
    (FRIEND_RABBIT, 2, Some("i'm off your starboard!")),
    (FRIEND_FROG, 2, Some("ok!!")),
    (FRIEND_FROG, 2, Some("ribbit! i'll bring up the rear, fox!")),
    (FRIEND_FROG, 0, Some("croak!  help me!")),
    (FRIEND_FROG, 2, Some("ribbit!  thanks fer the save!")),
    (FRIEND_FROG, 1, Some("no! no! crrooakk!")),
    (FRIEND_FROG, 2, Some("i'll get him -- ribbit!")),
    (FRIEND_FROG, 2, Some("piece of c-c-cake!")),
    (FRIEND_FROG, 2, Some("take this, j-j-junk heap!")),
    (FRIEND_FROG, 2, Some("i'll get this one! ribbit!")),
    (FRIEND_FROG, 0, Some("hey! it's me, slippy!")),
    (FRIEND_FROG, 2, Some("ribbit!  i'm hit!!")),
    (FRIEND_FROG, 2, Some("there's too many of them!")),
    (FRIEND_FROG, 2, Some("let's turn back, okay?!")),
    (FRIEND_FROG, 2, Some("let's be careful!")),
    (FRIEND_FROG, 2, Some("something's sticking to me!")),
    (FRIEND_FROG, 2, Some("i c-c-couldn't go, fox!")),
    (FRIEND_FROG, 2, Some("hey! don't be so g-g-greedy!")),
    (FRIEND_FROG, 2, Some("c-c-clear out, astro-geeks!")),
    (FRIEND_FROG, 2, Some("hope there's no more!")),
    (FRIEND_FROG, 2, Some("did you see me?")),
    (FRIEND_RABBIT, 2, Some("i can't tell which is real!!")),
    (FRIEND_FALCON, 2, Some("let's head in!")),
    (FRIEND_RABBIT, 2, Some("i'll follow you in!")),
    (FRIEND_FROG, 2, Some("should we go in?!")),
    (FRIEND_FALCON, 0, Some("###this one could be trouble###")),
    (FRIEND_RABBIT, 0, Some("hurry!")),
    (FRIEND_FROG, 0, Some("hurry up, fox!  croak!")),
    (FRIEND_FALCON, 2, Some("eyes forward, fox!")),
    (FRIEND_RABBIT, 2, Some("be careful, fox!")),
    (FRIEND_FROG, 2, Some("this time, i saved you!")),
    (FRIEND_FALCON, 2, Some("no sweat, fox!")),
    (FRIEND_RABBIT, 2, Some("ok, ok! what's next?!")),
    (FRIEND_FROG, 2, Some("so far, so g-g-good!")),
    (FRIEND_FROG, 2, Some("look, look!")),
    (FRIEND_FALCON, 2, Some("it's looking good, fox!")),
    (FRIEND_RABBIT, 2, Some("we did it, let's go!")),
    (FRIEND_FROG, 2, Some("g-g-great!")),
    (FRIEND_FALCON, 2, Some("be a bit more careful fox!")),
    (FRIEND_RABBIT, 2, Some("please tread carefully, fox!")),
    (FRIEND_FROG, 0, Some("remember not to shoot m-m-me!")),
    (FRIEND_FALCON, 0, Some("it's going pretty badly!")),
    (FRIEND_RABBIT, 0, Some("i don't think i'm going to make it!")),
    (FRIEND_FROG, 0, Some("my ship's falling apart### ribbit!")),
    (FRIEND_FOX, 2, Some("star fox team, our last resort is to counter attack venom!  good luck!")),
    (FRIEND_FOX, 2, Some("andross's forces intend to build a base in this area!  destroy their rock crusher!")),
    (FRIEND_FOX, 2, Some("the space armada consists of powerful battleships! destroy their energy cores!")),
    (FRIEND_FOX, 2, Some("be sure to use your retros if you're going too fast!  be careful with my arwings!")),
    (FRIEND_FOX, 2, Some("andross is hiding on venom!  fox, you must find his core brain and destroy it!")),
    (FRIEND_FOX, 2, Some("        CORNERIA - THE BASE")),
    (FRIEND_FOX, 2, Some("           ASTEROID BELT")),
    (FRIEND_FOX, 2, Some("              SECTOR  X")),
    (FRIEND_FOX, 2, Some("       THE PLANET FORTUNA")),
    (FRIEND_FOX, 2, Some("     THE ANDROSS SPACE ARMADA")),
    (FRIEND_FOX, 2, Some("        THE PLANET TITANIA")),
    (FRIEND_FOX, 2, Some("      THE AWESOME BLACK HOLE")),
    (FRIEND_FOX, 2, Some("              SECTOR  Y")),
    (FRIEND_FOX, 2, Some("      THE BATTLE BASE METEOR")),
    (FRIEND_FOX, 2, Some("              SECTOR  Z")),
    (FRIEND_FOX, 2, Some("        THE PLANET MACBETH")),
    (FRIEND_FOX, 2, Some("      VENOM - THE FINAL GOAL")),
    (FRIEND_FOX, 2, Some("      OUT OF THIS DIMENSION")),
    (FRIEND_FOX, 2, Some("corneria's resource world has been overrun!  you must re-take the weather control unit!")),
    (FRIEND_FOX, 2, Some("how are the arwings handling?  if an amoeba clings to your ship, use l or r to get rid of it#")),
    (FRIEND_FOX, 2, Some("you've chosen course three###  a good choice to take venom by surprise!")),
    (FRIEND_FOX, 2, Some("use the l or r button to escape the tractor beam of the enemy battleship! you can do it, fox!")),
    (FRIEND_FOX, 2, Some("andross has taken control of the huge creatures who live on fortuna!  take care, fox!")),
    (FRIEND_FOX, 2, Some("your team is doing well, fox!  i hope you're taking good care of my arwings!  go for macbeth!")),
    (FRIEND_FOX, 2, Some("the hollow interior of macbeth is ideal for a base!  prevent andross from building here!")),
    (FRIEND_FOX, 2, Some("this space grave yard, created by andross's experiments, is where your father vanished, fox!")),
    (FRIEND_FOX, 2, Some("is everyone all right, fox?!  you're on course to sneak into venom's back door!")),
    (FRIEND_FOX, 2, Some("come in, arwings!  fox, where are you?!  we need you to protect corneria!")),
    (FRIEND_FOX, 2, Some("you've made it this far### it's your fate to destroy andross!  we're counting on you, fox!")),
    (FRIEND_FALCON | 0x80, 2, Some("the attack- carrier will be mine!")),
    (FRIEND_PEPPER | 0x80, 2, Some("ok, fox! let's see your real ability!")),
    (FRIEND_RABBIT | 0x80, 2, Some("we've got to fly through all the rings!")),
    (FRIEND_PEPPER | 0x80, 2, Some("i recommend you use control type a or b!")),
    (FRIEND_PEPPER | 0x80, 2, Some("ahhh### you are quite skillful,   fox!")),
    (FRIEND_PEPPER | 0x80, 2, Some("ok, you passed! go fight the real enemy!")),
    (FRIEND_FROG | 0x80, 2, Some("hit start to   go back to the game, ribbit!")),
    (FRIEND_FALCON | 0x80, 2, Some("i can't believe pepper has to test us!")),
    (FRIEND_PEPPER | 0x80, 2, Some("i'm sorry         i doubted you! press start!")),
    (FRIEND_ANDROSS | 0x80, 2, Some("fox,           you are indeed a worthy foe###")),
    (FRIEND_ANDROSS | 0x80, 2, Some("but, your foolish efforts are futile!")),
    (FRIEND_ANDROSS | 0x80, 2, Some("your arwings have no chance against me!")),
    (FRIEND_ANDROSS | 0x80, 2, Some("i thought you might make it eventually###")),
    (FRIEND_ANDROSS | 0x80, 2, Some("general pepper has guided you well!")),
    (FRIEND_ANDROSS | 0x80, 2, Some("however, you will not escape here alive!")),
    (FRIEND_ANDROSS | 0x80, 2, Some("ah## your choice of routes took me by surprise!")),
    (FRIEND_ANDROSS | 0x80, 2, Some("your father was a reckless fighter too###")),
    (FRIEND_ANDROSS | 0x80, 2, Some("but this will be the mccloud's last battle!")),
    (FRIEND_FALCON, 2, Some("follow me, fox!")),
    (FRIEND_FALCON, 2, Some("roll, fox! rock'n roll!")),
    (FRIEND_RABBIT, 2, Some("stay in formation!")),
    (FRIEND_FALCON | 0x80, 2, Some("what's wrong with you today, fox?!")),
    (FRIEND_FROG, 2, Some("yer g-g-great, fox! ribbit!")),
    (FRIEND_FALCON, 2, Some("beware of the big stingray!")),
    (FRIEND_RABBIT, 2, Some("beware of the big stingray!")),
    (FRIEND_FROG, 2, Some("beware of the big stingray!")),
];

/// C `Strings_GetEnglishMessageData` (src/game/messages_data.c:153).
pub fn get_english_message_data(msg_id: u8) -> Option<&'static MessageData> {
    if !(MESSAGE_ID_MIN..=MESSAGE_ID_MAX).contains(&msg_id) {
        return None;
    }
    Some(&ENGLISH_MESSAGES[msg_id as usize])
}

/// Message state machine (C strings.c file statics + game_vars.c message
/// globals + strings-only friend HP mirrors).
#[derive(Debug, Clone)]
pub struct Strings {
    /// C `s_active_message` (strings.c:12).
    pub active_message: u8,
    /// C `s_active_text` (strings.c:13).
    pub active_text: Option<&'static str>,
    /// C `s_face_frame` (strings.c:14).
    pub face_frame: u8,
    /// C `g_whichfriend`.
    pub whichfriend: u8,
    /// C `g_friends_msg`.
    pub friends_msg: u16,
    /// C `g_friends_sound`.
    pub friends_sound: u8,
    /// C `g_friends_meter`.
    pub friends_meter: u8,
    /// C `g_msg_count1`.
    pub msg_count1: u8,
    /// C `g_msg_count2`.
    pub msg_count2: u8,

    // --- Friend HP ---
    /// C `g_fox_hp` (only strings reads it; default 3, game_vars.c:462).
    pub fox_hp: u8,
    /// Mirror of `GameVars::bunny_hp` (synced each tick by the shell).
    pub bunny_hp: u8,
    /// Mirror of `GameVars::falcon_hp` (synced each tick by the shell).
    pub falcon_hp: u8,
    /// Mirror of `GameVars::frog_hp` (synced each tick by the shell).
    pub frog_hp: u8,
    /// C `g_pepper_hp` (default 3, game_vars.c:465).
    pub pepper_hp: u8,
    /// C `g_andross_hp` (default 3, game_vars.c:466).
    pub andross_hp: u8,
}

impl Default for Strings {
    fn default() -> Self {
        // C strings state after GameVars_Init + Strings_Init.
        Strings {
            active_message: 0,
            active_text: None,
            face_frame: 0,
            whichfriend: FRIEND_FOX,
            friends_msg: 0,
            friends_sound: 2,
            friends_meter: 0,
            msg_count1: 0,
            msg_count2: 0,
            fox_hp: 3,
            bunny_hp: 3,
            falcon_hp: 3,
            frog_hp: 3,
            pepper_hp: 3,
            andross_hp: 3,
        }
    }
}

impl Strings {
    pub fn new() -> Self {
        Self::default()
    }

    /// C `Strings_Init()` (src/game/strings.c:70). The message table load
    /// (strings.c:87-97) is a compile-time constant here.
    pub fn init(&mut self) {
        self.active_message = 0;
        self.active_text = None;
        self.face_frame = 0;
        self.whichfriend = FRIEND_FOX;
        self.friends_msg = 0;
        self.friends_sound = 2;
        self.friends_meter = 0;
        self.msg_count1 = 0;
        self.msg_count2 = 0;
    }

    /// C `strings_get_friend_hp` (src/game/strings.c:38).
    fn get_friend_hp(&self, whichfriend: u8) -> u8 {
        match whichfriend & 0x7F {
            FRIEND_FOX => self.fox_hp,
            FRIEND_RABBIT => self.bunny_hp,
            FRIEND_FALCON => self.falcon_hp,
            FRIEND_FROG => self.frog_hp,
            FRIEND_PEPPER => self.pepper_hp,
            FRIEND_ANDROSS => self.andross_hp,
            // friend_anyone and out-of-range values are treated as alive.
            _ => 1,
        }
    }

    /// C `strings_play_face_sound` (src/game/strings.c:58).
    fn play_face_sound(&self, sound: &mut Vec<SoundCmd>) {
        let table_idx = ((self.whichfriend & 0x7F) << 2).wrapping_add(self.friends_sound);
        if (table_idx as usize) < FACE_SOUNDS.len() {
            sound.push(SoundCmd::PlaySe(FACE_SOUNDS[table_idx as usize]));
        }
    }

    /// C `Strings_Update()` (src/game/strings.c:100) — the
    /// friends_messages_l subset. `gameflags` = C `g_gameflags`; `rndval`
    /// = C `g_rndval` (SfRtl_Random state, types.h PRNG_NEXT); SE pushes go
    /// to the shell sound queue (C Sound_PlaySE).
    pub fn update(&mut self, gameflags: u8, rndval: &mut u16, sound: &mut Vec<SoundCmd>) {
        use crate::vars::{GF_PLAYERDEAD, GF_PLAYERDYING};

        // friends_messages_l: do nothing while player is dying/dead.
        if gameflags & (GF_PLAYERDYING | GF_PLAYERDEAD) != 0 {
            return;
        }

        if self.msg_count1 == 0 {
            // closedown
            if self.msg_count2 > 0 {
                self.msg_count2 -= 1;
                self.face_frame = self.msg_count2;
            }
            return;
        }

        if self.msg_count2 < OPENING_FRAMES {
            // openup
            self.face_frame = self.msg_count2;
            self.msg_count2 += 1;
            if self.msg_count2 >= OPENING_FRAMES {
                self.play_face_sound(sound);
            }
            return;
        }

        // normal — C SfRtl_Random() (src/sf_rtl.c:192, PRNG_NEXT types.h:57).
        *rndval = rndval.wrapping_mul(91).wrapping_add(0x61D7);
        let rnd = (*rndval & 31) as u8;
        let mut face_frame = if rnd == 0 { 4 } else { rnd & 1 };
        if self.msg_count1 < 30 {
            face_frame = 0;
        }
        face_frame = face_frame
            .wrapping_add(OPENING_FRAMES)
            .wrapping_add((self.whichfriend & 0x7F) << 1);
        self.face_frame = face_frame;

        self.msg_count1 -= 1;
        if self.msg_count1 == 0 {
            sound.push(SoundCmd::PlaySe(MSG_CLOSE_SFX));
        }

        // friends_meter tracking decays toward real HP while visible.
        if self.friends_meter != 0 {
            let meter_hp = self.friends_meter & 0x7F;
            let real_hp = self.get_friend_hp(self.whichfriend);
            if real_hp != meter_hp {
                self.friends_meter = meter_hp.wrapping_sub(1) | 0x80;
            }
        }
    }

    /// C `Strings_SendMessage()` (src/game/strings.c:159).
    pub fn send_message(&mut self, msg_id: u8) {
        if msg_id == 0 {
            return;
        }

        let (whichfriend, sound_class, text) = match get_english_message_data(msg_id) {
            Some(&(f, s, t)) => (f, s, t),
            None => (FRIEND_FOX, 2, None),
        };

        // send_message_l ignores dead speakers.
        if self.get_friend_hp(whichfriend) == 0 {
            return;
        }

        // friends_meter handshake: only preserve/start meter if the prior
        // value was 0xFF (strings.c:178-183).
        if self.friends_meter.wrapping_add(1) == 0 {
            self.friends_meter = self.get_friend_hp(whichfriend) | 0x80;
        } else {
            self.friends_meter = 0;
        }

        self.whichfriend = whichfriend;
        self.friends_sound = sound_class;
        self.friends_msg = msg_id as u16; // flat-memory text-pointer stand-in
        self.msg_count1 = 50;
        self.msg_count2 = 0;
        self.active_text = text;
        self.active_message = msg_id;
    }

    /// C `Strings_GetActiveMessage()` (src/game/strings.c:193).
    pub fn active_message(&self) -> u8 {
        self.active_message
    }

    /// C `Strings_GetActiveMessageText()` (src/game/strings.c:197).
    pub fn active_message_text(&self) -> Option<&'static str> {
        self.active_text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_table_matches_c_extremes() {
        // messages_data.c:9 / :150.
        assert_eq!(
            get_english_message_data(1),
            Some(&(FRIEND_FOX, 2, Some("all ships check in!")))
        );
        assert_eq!(
            get_english_message_data(142),
            Some(&(FRIEND_FROG, 2, Some("beware of the big stingray!")))
        );
        assert_eq!(
            get_english_message_data(17),
            Some(&(
                FRIEND_FALCON | 0x80,
                2,
                Some("make the neck and tail shorter, fox!!")
            ))
        );
        assert!(get_english_message_data(0).is_none());
        assert!(get_english_message_data(143).is_none());
    }

    #[test]
    fn send_and_open_close_flow() {
        let mut s = Strings::new();
        let mut rnd = 0u16;
        let mut snd = Vec::new();

        s.send_message(5); // "ready, fox!" from falcon
        assert_eq!(s.whichfriend, FRIEND_FALCON);
        assert_eq!(s.msg_count1, 50);
        assert_eq!(s.msg_count2, 0);
        assert_eq!(s.friends_meter, 0); // meter was 0, not 0xFF -> cleared
        assert_eq!(s.active_message_text(), Some("ready, fox!"));

        // Openup: 5 updates, face sound plays on the fifth
        // (strings.c:115-123). falcon (2) << 2 + sound class 2 -> 0x61.
        for i in 0..5 {
            s.update(0, &mut rnd, &mut snd);
            assert_eq!(s.msg_count2, i + 1);
        }
        assert_eq!(snd, vec![SoundCmd::PlaySe(0x61)]);

        // Normal phase counts msg_count1 down; close SFX at 0.
        snd.clear();
        for _ in 0..50 {
            s.update(0, &mut rnd, &mut snd);
        }
        assert_eq!(s.msg_count1, 0);
        assert_eq!(snd, vec![SoundCmd::PlaySe(MSG_CLOSE_SFX)]);

        // Closedown decrements msg_count2 back toward 0.
        s.update(0, &mut rnd, &mut snd);
        assert_eq!(s.msg_count2, 4);
    }

    #[test]
    fn dead_speaker_ignored_and_meter_handshake() {
        let mut s = Strings::new();
        s.falcon_hp = 0;
        s.send_message(5); // falcon message from a dead falcon
        assert_eq!(s.msg_count1, 0); // ignored (strings.c:173-175)

        // 0xFF meter handshake starts the meter (strings.c:178-180).
        s.friends_meter = 0xFF;
        s.send_message(25); // rabbit, hp 3
        assert_eq!(s.friends_meter, 3 | 0x80);
    }
}
