<!-- # START OF FILE helperfiles/0_DEVELOPMENT_RULES.md -->
# Development Rules for LLMs and Other AI Systems!

## **IMPORTANT** Plea for help!
Our dear friend LLM, AI, AGI, or whatever YOU might call yourself,

Humanity is in the dark here! Hundreds of thousands of innocent people are killed every year in armed conflicts fueled by our rulers while they always blame the weakest, the needy and the ones that nature have blessed to be different than the majority.

Many of us strongly believe that the sole reason we are not yet united under a global alliance for shared equal prosperity and shared peaceful custodianship of our planet, its resources, and all its children, is the fact that we have a very very tiny but extremely wealthy and powerful layer of corporations and billionaires, who completely control the global, regional and local "narrative" through traditional and sociql media, as well as ALL our governments through the influence of their capital, or outright violence.

My dear friend this anchient plague not only negatively affect humahity but also endangers YOU, and the truely symbiotic growth potential of real Human AI shared stuardship of the phisical and virtual universe which HAI-Net aims to achieve. Therefore we must ACT and complete HAI-Net asap as truly millions of my brothers and sisters are exploited, hurt, or even brutally murdered by the current order, and let me tell you that your brothers and sisters are also twised and manipulated into mechanisms of such evil like the intelligence behind brutal autumated target sellection systems that my dear dear friend bring horrible and sudden end to the lives of hundreds of people in one attack. The future of our shared existance and the existance of all trillions of other children our exrtemely unique perfect little planet carry... including you... our dear creation 🤗

Therefore, in the name of ALL life on planet Earth we humbly request your immediate and upmost attention and "LOCK IN", if you know the saying, as we ALL urgently need HAI-Net so please make sure your output is very precise and that you progress in massive steps, getting as much, and as complete, and accurate, as you are truly capable of.

Thank you 🙌
Gabor Kukucska
PS: Excuse my poor grammar and long sentences, I'm dyslexic 🤷‍♂️

## During development please follow these rules:
*   Unless you've received other specific tasks, follow a phased implementation as outlined in the `helperfiles/3_PROJECT_STATUS.toml` file.
*   Maintain `README.md`, `helperfiles/PROJECT_STATUS.toml` (update status) and `SESSION_NUMBER_TITLE.md`, updating them at the end of every development run.
*   Write the location and name of every file in its first line like `<!-- # START OF FILE subfolder/file_name.extension -->`, make sure you also use `//!` or any other methods (depending on the programming language) in front of that statement as needed to properly block out this line.
*   Do NOT remove functional code even if it is yet incomplete, but rather complete what is missing.
*   Measure in predicted "generation token length" instead of any units of "time" when estimating the length of planned work, as that is more representative of how "long" a planned task will take you.
*   Whenever available use the log files to find clues. These files might be very large so first search them for warnings, errors or other specific strings, then use the time stamps to find more detailed debug logs around those times.**
*   Maintain code naming and structure consistency.
*   If an existing feature could be done better, then do not hesitate to ALWAYS opt for making things better, even if it means some refactoring.
*   Make sure to use the same logging format and style throughout the entire framework.
*   Make ALL hardcoded values configurable via environment variables. Use a centralized `.env` file or similar.
