use crate::Result;
#[cfg(not(target_os = "macos"))]
pub fn run() -> Result {
    Err("spellfilter requires the macOS English spell checker".into())
}
#[cfg(target_os = "macos")]
pub fn run() -> Result {
    use objc2::{msg_send, rc::autoreleasepool};
    use objc2_app_kit::NSSpellChecker;
    use objc2_foundation::{NSNotFound, NSRange, NSString};
    use std::io::{self, BufRead};
    use unicode_segmentation::UnicodeSegmentation;
    autoreleasepool(|_| unsafe {
        let checker = NSSpellChecker::sharedSpellChecker();
        let _: () = msg_send![&*checker,setAutomaticallyIdentifiesLanguages:false];
        let _: bool = msg_send![&*checker,setLanguage:&*NSString::from_str("en")];
        for line in io::stdin().lock().lines() {
            let line = line?;
            let word = line.trim();
            if word.is_empty() {
                continue;
            }
            let mut clusters = word.graphemes(true);
            let capitalized = clusters.next().unwrap().to_uppercase() + clusters.as_str();
            for form in [word, capitalized.as_str()] {
                let range: NSRange = msg_send![&*checker,checkSpellingOfString:&*NSString::from_str(form),startingAt:0isize,language:&*NSString::from_str("en"),wrap:false,inSpellDocumentWithTag:0isize,wordCount:std::ptr::null_mut::<isize>()];
                if range.location == NSNotFound as usize {
                    println!("{word}");
                    break;
                }
            }
        }
        Ok(())
    })
}
