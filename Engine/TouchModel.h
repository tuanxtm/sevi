//
//  TouchModel.h
//  Sevi
//
//  Fat-finger model for touch keyboards: which key did a touch mean? Combines where the finger
//  landed (2D Gaussian around each key's center) with how likely each key is next given the letters
//  of the word so far (vAutoSwitchNextKeyMass), mixed by the current language mode. A touch well
//  inside a key always types that key; the model only decides touches near key borders.
//

#ifndef SeviTouchModel_h
#define SeviTouchModel_h

struct TMKey {
    char letter;          // 'a'..'z'
    float x, y, w, h;     // frame, any unit (points)
};

struct TMCandidate {
    char letters[32];        // the keys of the word (Telex keys for Vietnamese)
    unsigned short text[48]; // what it looks like
    int textLen;
    int vietnamese;
    double score;            // log probability (higher is better)
};

class TouchModel {
public:
    /** The letter for a touch at (x, y). prefix: letters typed so far in this word (lowercase).
     *  vietnamese: current mode. keys: the letter keys of the layout. */
    static char pick(float x, float y, const char* prefix, int vietnamese, const TMKey* keys, int n);
    /** Auto-correct: candidates for a finished word, best first (edits of one key, scored by word
     *  frequency, where each key was touched and the mode). touchX/touchY: one per letter, or null.
     *  Returns how many (<= max). The typed word itself is included when it is a word. */
    static int candidates(const char* typed, const float* touchX, const float* touchY, int vietnamese,
                          const TMKey* keys, int n, TMCandidate* out, int max);
    /** Whether to replace the typed word with candidates[0] at the space (conservative). */
    static bool shouldCorrect(const char* typed, const TMCandidate* c, int count);
    // tuning (see Tools/autoswitch/touch: chosen by simulation)
    static float sigma;        // finger spread, in key widths
    static float sigmaY;       // finger spread up/down, in key heights
    static float weight;       // how much the prior counts (0 = nearest key only)
    static float sureZone;     // inside this fraction of a key, the key wins outright
    static float modeShare;
    static float correctMargin;
    static float deleteCost;        // log cost of an extra key (the word without it)
    static float correctFloor;      // the fix must score above this (3–4 letter words)
    static float correctFloorLong;  // … and this for 6+ letters (5: halfway)
    static const char* keepWords;  // never "corrected": chat shorthand etc., newline-separated, lowercase  // auto-correct only when the best candidate leads by this much (log)    // share of the current mode's language in the prior (the rest: the other language)
};

#endif
