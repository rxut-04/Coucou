// Motivational quotes for different workflow contexts and sessions
// 50+ quotes per session category and 60+ quotes for browser

export type QuoteCategory =
  | "vscode"
  | "antigravity"
  | "kiro"
  | "claude"
  | "chatgpt"
  | "konsole"
  | "terminal"
  | "browser"
  | "general";

export const WORKFLOW_QUOTES: Record<QuoteCategory, string[]> = {
  vscode: [
    "Code is like humor. When you have to explain it, it's bad. 💡",
    "First, solve the problem. Then, write the code. 🎯",
    "Clean code always looks like it was written by someone who cares. ✨",
    "Make it work, make it right, make it fast. ⚡",
    "The best error message is the one that never shows up. 🛡️",
    "Simplicity is the soul of efficiency. 🧠",
    "Any fool can write code that a computer can understand. Good programmers write code that humans can understand. 📖",
    "Programming isn't about what you know; it's about what you can figure out. 🔍",
    "The only way to learn a new programming language is by writing programs in it. 💻",
    "Sometimes it's better to leave something alone, to pause, and that's very true of programming. ⏸️",
    "Testing leads to failure, and failure leads to understanding. 🧪",
    "Good code is its own best documentation. 📝",
    "Debugging is twice as hard as writing the code in the first place. 🐛",
    "Perfection is achieved not when there is nothing more to add, but rather when there is nothing more to take away. 🎨",
    "Code never lies, comments sometimes do. 🔍",
    "Every function you refine today saves an hour of debugging tomorrow. 🛠️",
    "Small, atomic commits build unstoppable momentum. 🚀",
    "Types are the guardrails that let you refactor at full speed. 🛤️",
    "Readability counts: your future self is your most important teammate. 🤝",
    "Delete dead code fearlessly; git remembers everything. 🧹",
    "Great architecture emerges from clear boundaries and simple interfaces. 🏛️",
    "Don't optimize prematurely—measure first, then cut with precision. 📏",
    "A well-named variable is worth a paragraph of comments. 🏷️",
    "When a function does one thing well, bugs have nowhere to hide. 🔦",
    "Refactoring is like tidying your workshop before the next masterpiece. 🧰",
    "Hard problems become easy when you find the right data structure. 🧩",
    "Write tests that give you confidence, not just coverage. ✅",
    "Every compiler warning you fix is a production incident prevented. 🚨",
    "Keep your modules decoupled and your logic cohesive. 🔗",
    "Deep focus in the editor turns complex specs into clean reality. 💎",
    "Master your keyboard shortcuts and let your thoughts flow into code. ⌨️",
    "Edge cases are where craftsmanship truly shines. 🌟",
    "A bug reproduced is already halfway fixed. 🔬",
    "Design for today's requirements with tomorrow's flexibility in mind. 🌱",
    "Consistent formatting frees your mind to focus on logic. 📐",
    "State management is clearest when data flows in one direction. 🌊",
    "Pure functions make reasoning effortless and testing a breeze. 🍃",
    "Don't repeat yourself, but don't over-abstract too early either. ⚖️",
    "The best pull request is small, focused, and easy to review. 📦",
    "Understand the root cause before writing a single line of fix. 🧭",
    "Let the type checker do the heavy lifting for you. 🏋️",
    "Shipping working software is the heartbeat of engineering. ❤️",
    "Complex systems work best when built from simple, composable parts. 🧱",
    "Patience in debugging beats frantic guessing every single time. 🧘",
    "Your IDE is your canvas—craft every module with intention. 🖼️",
    "Handle errors gracefully; resilience separates toys from tools. 🛡️",
    "Keep side effects at the edges and keep your core logic pure. 🎯",
    "One focused hour in VS Code beats a whole day of context switching. 🔥",
    "Leave every file a little cleaner than you found it. ✨",
    "Clear interfaces make team collaboration effortless. 🌐",
    "When stuck, step back and sketch the data flow on paper. ✏️",
    "Performance comes from doing less work, not just faster work. 🏎️",
    "Celebrate every green test suite—you earned it! 🏆",
    "Great software is built one thoughtful line at a time. 🧱",
    "Stay curious, keep building, and trust your engineering instincts. 🚀",
  ],

  antigravity: [
    "Defy complexity—build agentic systems that feel effortless. 🪐",
    "Antigravity mode: lifting heavy engineering tasks off your shoulders! 🚀",
    "Human creativity + agentic precision = unstoppable velocity. ⚡",
    "Think at the architecture level and let your workflows fly. 🌌",
    "High-leverage engineering starts with crystal-clear intent. 🎯",
    "When your tools think with you, no codebase is too large. 🧠",
    "Zero gravity, maximum focus—keep shipping bold ideas! 🛸",
    "Orchestrate complex refactors with calm, systematic precision. 🎼",
    "Great prompts and sharp engineering intuition make magic happen. ✨",
    "Elevate your workflow: design the system, verify the details. 🏗️",
    "Fast iteration loops turn ambitious visions into working code. 🔄",
    "Break massive milestones into clean, verifiable steps. 🪜",
    "You are the system architect—steer the mission with confidence! 🧭",
    "Every automated check gives you more freedom to innovate. 🛡️",
    "Let curiosity guide your exploration and rigor guide your commits. 🔬",
    "Agentic pair programming: twice the insight, half the friction. 🤝",
    "Stay in flow state while complex tasks assemble around you. 🌊",
    "Verify, refine, and ship—excellence is in the feedback loop. ✅",
    "Build tools that amplify your intelligence every single day. 🔋",
    "No boilerplate can weigh you down in Antigravity! 🪶",
    "From hypothesis to working prototype in record time. ⏱️",
    "Deep context awareness turns tough bugs into quick wins. 💡",
    "Architect for scale, test for reliability, polish for joy. 💎",
    "Keep your eyes on the big picture while nailing every edge case. 🔭",
    "Momentum is everything—keep the build green and moving forward! 🟢",
    "Smart delegation lets you focus on the hardest design decisions. 🧩",
    "Turn multi-file refactors into smooth, fearless upgrades. 🛠️",
    "Clarity of thought translates directly into clarity of code. 📖",
    "Explore bold solutions—you have the safety net to experiment! 🧪",
    "Precision engineering meets weightless productivity. 🌟",
    "Every problem has an elegant solution waiting to be uncovered. 🗝️",
    "Keep pushing the frontier of what one developer can build. 🏔️",
    "Seamless context, sharp execution, extraordinary results. 🏆",
    "Review with an architect's eye and ship with a builder's pride. 🏛️",
    "Your focus sets the trajectory—aim high and keep building! 🚀",
    "Complex debugging becomes a guided expedition in Antigravity. 🔦",
    "Write less ceremonial code and more high-impact logic. ⚡",
    "Continuous verification keeps your velocity sustainable. 📈",
    "Great systems are forged when deep thinking meets rapid execution. 🔥",
    "Transform rough ideas into production-grade architecture. 🏗️",
    "Stay sharp, stay curious, and let your productivity soar! 🦅",
    "Clean abstractions make even the biggest codebase feel light. ☁️",
    "One well-structured plan saves ten hours of trial and error. 🗺️",
    "Harness agentic power to conquer technical debt for good. 🧹",
    "You're building the future one intelligent step at a time. 🌐",
    "Keep the feedback loop tight and the code quality high. 🎯",
    "Weightless coding: all the signal, none of the noise. 🎧",
    "Solve the hard architectural puzzles and enjoy the craft. 🧠",
    "Every session in Antigravity brings your vision closer to launch. 🚀",
    "Bold ideas deserve fearless execution—keep going! 💪",
    "Master the art of high-level direction and deep-dive verification. 🔍",
    "Your engineering flow is dialed in—ride the wave! 🌊",
    "Build cleanly, test thoroughly, and ship with confidence. ✨",
    "Nothing is out of reach when your tooling matches your ambition. 🌠",
    "Keep soaring—your best engineering work is happening right now! 🪐",
  ],

  kiro: [
    "Spec-driven development turns ambiguity into rock-solid code. 📋",
    "Kiro flow: align requirements, design, and tasks before you build! 🎯",
    "Clear specifications are the blueprint of bug-free software. 🏗️",
    "When your design doc and code stay in sync, magic happens. ✨",
    "Break every feature into crisp, testable acceptance criteria. ✅",
    "Think through the edge cases in the spec, breeze through the code. 🧠",
    "Structured engineering beats guesswork every day of the week. 📐",
    "From user story to architecture to implementation—step by step! 🪜",
    "Great specs make complex features feel straightforward. 💡",
    "Keep your tasks granular and your progress unmistakable. 📈",
    "Design first, code second, ship with total confidence. 🚀",
    "A well-crafted requirement prevents a dozen late-stage rewrites. 🛡️",
    "Let Kiro help you turn product vision into clean execution. ⚡",
    "Every completed task in your plan builds unstoppable momentum. 🔥",
    "Traceability from spec to test makes maintenance effortless. 🔗",
    "Clear acceptance criteria tell you exactly when you're done. 🏁",
    "Good system design is the ultimate productivity multiplier. 🏛️",
    "Take a moment to refine the plan—it pays back tenfold in code. 🧭",
    "Systematic builders create the most reliable software. 🧱",
    "Your roadmap is clear; execute one focused task at a time. 🎯",
    "Hooks and specs keep your codebase consistent automatically. 🤖",
    "Document the 'why' in your design so the 'how' stays obvious. 📖",
    "Small, verified increments lead to massive engineering wins. 🏆",
    "Keep your architecture modular and your specs living. 🌱",
    "When requirements are crystal clear, coding is pure joy. 💎",
    "Check off that next task—you're making steady, solid progress! ✔️",
    "Bridge the gap between product thinking and systems engineering. 🌉",
    "Thoughtful planning eliminates last-minute surprises. 🔦",
    "Stay disciplined with your workflow and watch quality soar. 🌟",
    "Every edge case caught in design is a bug that never existed. 🐛",
    "Build with intention, verify against the spec, and ship proud. 🚀",
    "Clean task breakdown turns intimidating projects into easy wins. 🧩",
    "Your Kiro session is humming—keep knocking out those tasks! 💪",
    "Precision in planning leads to speed in delivery. 🏎️",
    "Let structured thinking be your unfair engineering advantage. 🧠",
    "Review the spec, trust the process, and write clean code. ✨",
    "Great software starts with asking the right design questions. 🔍",
    "Keep your momentum steady and your implementation faithful. ⚓",
    "Modular design today means effortless scaling tomorrow. 📦",
    "Turn complex domain logic into clear, verifiable rules. 📏",
    "One task at a time—watch the whole system come together! 🏗️",
    "Consistency across specs, tests, and code is pure craftsmanship. 🛠️",
    "Focus on the current milestone and nail the execution. 🎯",
    "Smart workflows free your mind for creative problem solving. 🎨",
    "Every green check in your task list is a step toward launch. 🟢",
    "Architect with clarity, implement with care, deliver with pride. 🏅",
    "When the plan is solid, deep focus comes naturally. 🧘",
    "Keep refining your craft—structured engineering is a superpower. ⚡",
    "Your attention to detail sets your software apart. 💎",
    "Stay in the zone and bring your system design to life! 🌟",
    "Clear inputs, predictable outputs, resilient architecture. 🛡️",
    "Step by step, spec by spec, you're building something great. 🚀",
    "Trust your blueprint and enjoy the rhythm of building. 🎵",
    "Quality isn't an accident—it's designed right from the start. 🏛️",
    "Keep going—your structured approach is paying off big time! 🔥",
  ],

  claude: [
    "Deep reasoning works best when paired with clear, thoughtful questions. 🧠",
    "Brainstorm boldly with Claude, then validate with real-world code! ✨",
    "The quality of your question shapes the quality of the insight. 🎯",
    "Use AI as a sounding board to sharpen your own architecture. 🏛️",
    "Explore three approaches in two minutes, then build the best one. 🚀",
    "Great engineers use Claude to challenge their assumptions. 🔍",
    "Turn complex concepts into crystal-clear mental models. 💡",
    "Draft, critique, refine—iteration is the secret to brilliance. 🔄",
    "Ask 'why' and 'what could go wrong' before committing to a design. 🛡️",
    "Synthesize big ideas into actionable engineering steps. 📋",
    "Let Claude handle the rough draft while you own the craftsmanship. 🛠️",
    "Clear context in, high-signal answers out. 📡",
    "Thinking through edge cases together saves hours of solo debugging. 🐛",
    "Use this session to deepen your understanding, not just copy code. 📖",
    "Second-order thinking separates good solutions from great ones. 🧭",
    "Break down gnarly algorithms into intuitive, step-by-step logic. 🧩",
    "Review every generated snippet with a senior engineer's eye. 🔬",
    "Great conversations spark the best architectural breakthroughs. ⚡",
    "Test the hypothesis right away—code is the ultimate truth! ✅",
    "Summarize complex docs into clear action items and keep moving. 📈",
    "When stuck on a rubber-duck problem, talk it through clearly. 🦆",
    "Combine broad knowledge with your deep project intuition. 🤝",
    "Refine your prose, sharpen your specs, elevate your code. 💎",
    "Curiosity + structured prompting = rapid mastery of any stack. 🌱",
    "Always verify the subtle details—you are the pilot in command! 🧑‍✈️",
    "Use counterexamples to stress-test your system design early. 🧪",
    "Turn vague ideas into concrete interfaces and data structures. 🏗️",
    "Fast research, deep comprehension, confident execution. 🏎️",
    "Ask for trade-offs so you can make informed architectural choices. ⚖️",
    "Every great solution starts with a well-framed problem. 🖼️",
    "Keep the dialogue focused on your core milestone today. 🎯",
    "Learn the underlying principle so you own the knowledge forever. 🧠",
    "From whiteboard thoughts to production-ready specs in minutes. ⏱️",
    "Spot hidden bottlenecks by walking through the data flow step by step. 🔦",
    "Pair your domain expertise with rapid analytical reasoning. 🌟",
    "Don't just fix the symptom—ask about the root cause! 🌳",
    "Clean abstractions come from discussing the problem from multiple angles. 📐",
    "Translate insights straight into your editor while they're fresh. 💻",
    "Thoughtful collaboration turns hard days into productive breakthroughs. 🔥",
    "Keep your prompts concise, specific, and grounded in constraints. 📏",
    "Explore unfamiliar APIs with confidence and curiosity. 🗺️",
    "A five-minute design review with Claude can save a week of rework. 🛡️",
    "Stay critical, stay curious, and keep building great things! 🚀",
    "Distill long logs and stack traces into the exact line that matters. 🎯",
    "Use analogies to master new paradigms in record time. 💡",
    "Great builders never stop asking better questions. ❓",
    "Turn theoretical ideas into practical, testable implementations. 🧱",
    "Keep your momentum high—take this insight and ship it! ⚡",
    "Clarity of language reflects clarity of architecture. 📜",
    "You're sharpening both your code and your engineering mind. 🏆",
    "Balance deep exploration with decisive execution. ⚖️",
    "Every tough problem yields to patient, structured reasoning. 🧘",
    "Capture the key takeaway and put it straight to work! 🛠️",
    "Smart engineers leverage every tool to think deeper and faster. 🧠",
    "Great progress in Claude—now let's turn those insights into code! 🚀",
  ],

  chatgpt: [
    "Fast answers are great; deep understanding is unbeatable. 🧠",
    "Use ChatGPT to accelerate your research, then build with precision! 🚀",
    "Great prompts include context, constraints, and a clear goal. 🎯",
    "Brainstorm ten possibilities, then engineer the one that scales. 🏗️",
    "Turn quick prototypes into clean, maintainable production code. ✨",
    "Verify every assumption with a quick test in your workspace. ✅",
    "Ask for edge cases and failure modes before you ship. 🛡️",
    "Speed up the boilerplate so you can focus on core architecture. ⚡",
    "A sharp question cuts straight through hours of documentation. 🔍",
    "Learn a new concept today and apply it within the next five minutes. 💡",
    "Don't just accept the first answer—ask for the cleanest trade-off! ⚖️",
    "Use AI to explore unfamiliar territory, then map it with your own code. 🗺️",
    "Rapid problem-solving works best when you stay in the driver's seat. 🏎️",
    "Break big questions into focused, testable sub-problems. 🧩",
    "Combine conversational speed with rigorous engineering standards. 💎",
    "Ask 'how can this be simpler?' to strip away accidental complexity. 🧹",
    "From blank page to working draft in seconds—now make it yours! 🛠️",
    "Use rubber-ducking in ChatGPT to untangle tricky logic knots. 🦆",
    "Always read every line of code before bringing it into your repo. 🔬",
    "Great builders turn information into action without delay. 🔥",
    "Compare two algorithms side by side and pick the right tool for the job. 📐",
    "Use quick explanations to unblock your flow and get back to building. 🌊",
    "Synthesize API patterns quickly, then write clean integration tests. 🧪",
    "Keep your session goal-oriented: get the insight, ship the feature! 🏁",
    "Curiosity powers discovery; discipline powers delivery. 💪",
    "Ask for a minimal reproducible example to master new libraries fast. 📦",
    "Turn error messages into learning moments that level up your skills. 📈",
    "Good prompts save minutes; good architecture saves months. 🏛️",
    "Use ChatGPT to draft the regex, then write unit tests to prove it! 🎯",
    "Stay focused on the problem you're solving right now. 🧭",
    "Translate high-level ideas into concrete TypeScript and Rust types. 🦀",
    "Quick iteration between chat and editor keeps your momentum flying. 🔄",
    "Challenge the solution: 'What happens under high load or bad input?' 🚨",
    "Distill complex specs into a clear checklist you can execute today. 📋",
    "Your engineering judgment is the secret ingredient AI can't replace. 🌟",
    "Unblock yourself fast and jump right back into deep work! ⚡",
    "Use examples and counterexamples to lock in new mental models. 🧠",
    "Keep your questions sharp and your implementation even sharper. 🗡️",
    "Explore creative angles, then ground them in solid engineering. 🌱",
    "Every new concept you master compounds over your entire career. 📚",
    "Don't get lost in endless prompting—take what works and build! 🧱",
    "Ask for time and space complexity to keep your code performant. ⏱️",
    "Turn documentation mysteries into clear, actionable knowledge. 🔦",
    "Great developers pair fast lookup with deep verification. 🤝",
    "Use this spark of inspiration to finish your current task strong! 🏆",
    "Simplify the logic until there's nowhere for bugs to hide. 🛡️",
    "Keep the feedback loop between idea and execution razor-tight. 🎯",
    "You have the answer you need—now go make it run! 💻",
    "Smart research + focused coding = unstoppable daily progress. 🚀",
    "Ask better questions, write cleaner code, ship faster features. ✨",
    "Stay curious, verify everything, and enjoy the build! 🔍",
    "Turn every roadblock into a stepping stone in minutes. 🪜",
    "Your focus is on point—keep driving this project forward! 🔥",
    "Blend rapid AI assistance with timeless software craftsmanship. 🛠️",
    "Great research session in ChatGPT—time to turn it into working code! 🚀",
  ],

  konsole: [
    "Konsole power user: KDE precision meets command-line mastery! 🖥️",
    "Split panes, fast tabs, pure Linux engineering flow in Konsole. ⚡",
    "Your KDE terminal is where system architecture meets bare metal. 🐧",
    "One crisp pipeline in Konsole beats a hundred clicks in a GUI. 🚀",
    "Master your shell history—`Ctrl+R` is a time machine for commands! ⏪",
    "Clean logs, green builds, and zero warnings in Konsole. ✅",
    "In Konsole, you have direct control over the entire operating system. 🛠️",
    "Automate the repetitive steps and keep your terminal workflow effortless. 🤖",
    "Every command you run with precision builds deeper Linux intuition. 🧠",
    "Watch your processes, inspect your sockets, own your environment. 🔍",
    "Konsole tabs organized, compiler flying, focus locked in! 🔥",
    "A well-tuned `.bashrc` or `.zshrc` turns Konsole into a superpower. ⚙️",
    "Pipe `grep`, `awk`, and `jq` together like a true Unix artisan. 🧩",
    "Check the exit code, verify the output, and ship with confidence. 🛡️",
    "Fast keyboard navigation in Konsole keeps you in pure flow state. ⌨️",
    "From `cargo build` to `git push`, your terminal is mission control. 🛸",
    "Read the stack trace carefully—Konsole is telling you the exact fix! 🔦",
    "Keep your scripts idempotent and your shell environment clean. 🧹",
    "Nothing beats the speed of a native KDE terminal session. 🏎️",
    "Monitor resource usage and keep your Linux system purring smoothly. 📈",
    "Small shell aliases save thousands of keystrokes over a year. 💡",
    "Test your CLI commands on a small sample before running at scale. 🧪",
    "Your Konsole session is dialled in—keep crushing those tasks! 💪",
    "Understand file permissions, signals, and streams like a pro. 📜",
    "Quick `git status`, clean `git diff`, thoughtful `git commit`. 📦",
    "Use Konsole search (`Ctrl+Shift+F`) to pinpoint log events instantly. 🎯",
    "Every daemon, service, and build bends to a skilled terminal user. 🏛️",
    "Stay calm when a build fails—the error log is your roadmap. 🗺️",
    "Combine `find` and `xargs` to transform whole directories in seconds. ⚡",
    "Your command line fluency makes you ten times more effective. 🌟",
    "Keep your working tree tidy and your branches well-named. 🌿",
    "Inspect the network, verify the port, confirm the payload. 🌐",
    "Konsole + Linux = the ultimate sandbox for serious systems work. 🐧",
    "Write shell scripts with `set -euo pipefail` for rock-solid safety. 🛡️",
    "Measure execution time with `time` and optimize what actually matters. ⏱️",
    "One terminal pane for logs, one for tests, one for git—pure harmony! 🎼",
    "Every manual fix you script today becomes tomorrow's one-liner. 📝",
    "Deep system knowledge comes from getting your hands dirty in Konsole. 🔬",
    "Keep your packages updated and your dependencies audited. 🔒",
    "A clean terminal prompt clears the mind for the next command. ✨",
    "Debug with `strace`, `lsof`, and `journalctl`—nothing stays hidden! 🕵️",
    "Your Linux workflow is sharp, fast, and completely under your control. 🎮",
    "Respect the power of the shell: double-check paths before `rm`! ⚠️",
    "Turn complex deployment steps into reproducible Makefile targets. 🏗️",
    "Watch those tests turn green right in your Konsole window! 🟢",
    "The command line rewards precision, patience, and curiosity. 🧭",
    "Use background jobs and process management like a seasoned sysadmin. 🔋",
    "Every flag you learn adds another tool to your utility belt. 🧰",
    "Fast feedback in the terminal keeps your coding momentum unstoppable. 🌊",
    "Log output is data—filter the noise and focus on the signal. 📡",
    "Build from source, verify the binary, celebrate the release! 🏆",
    "Your Konsole mastery turns tough devops chores into quick wins. ⚡",
    "Stay focused in the shell—you're making real systems progress! 🔥",
    "Linux craftsmanship shines brightest in the terminal. 💎",
    "Keep typing with purpose—Konsole is ready for your next command! 🚀",
  ],

  terminal: [
    "The terminal is where real magic happens. ✨",
    "Command line mastery is a superpower. 💪",
    "Automate the boring stuff, focus on what matters. 🤖",
    "Every command brings you closer to your goal. 🎯",
    "Shell scripting: turning hours into seconds. ⚡",
    "In the terminal, you are in complete control. 🎮",
    "Efficiency is doing things right; effectiveness is doing the right things. 📈",
    "The best developers know their tools inside out. 🔧",
    "Command by command, you're building something great. 🏗️",
    "Master the terminal, master your workflow. 🚀",
    "Small utilities composed with pipes can solve giant problems. 🔗",
    "A fast shell and a sharp mind are all a builder needs. 🧠",
    "Let `git log` tell the story of your project's evolution. 📖",
    "Automate once, benefit a thousand times. 🔄",
    "Read the man page for two minutes to save two hours of guessing. 📚",
    "Clean stdout, quiet stderr, exit code 0—perfection! ✅",
    "Your terminal is the shortest path between intention and execution. ⚡",
    "Use `ripgrep` and `fd` to navigate massive codebases at lightning speed. 🏎️",
    "Every shell one-liner you craft sharpens your data-wrangling skills. 🧩",
    "Keep your dotfiles versioned—your environment is your craft. 🛠️",
    "Test your scripts in a safe directory before running in production. 🛡️",
    "Watch your test runner in real time and fix regressions on the spot. 🔬",
    "The command line never goes out of style—it only makes you faster. ⏱️",
    "Use `tmux` or tabs to keep your servers, logs, and shells organized. 🗂️",
    "When a service misbehaves, the terminal always tells the truth. 🔦",
    "Master standard streams: stdin, stdout, and stderr are your best friends. 🌊",
    "A clean `git status` at the end of a feature feels amazing. 🌟",
    "Write clear commit messages—your future self will thank you! 📝",
    "Use environment variables wisely and keep secrets out of history. 🔒",
    "From local dev server to production deploy, the CLI powers it all. 🌐",
    "Check disk space, memory, and CPU with a single keystroke. 📊",
    "Don't fear the stack trace; read it from the bottom or top with calm focus. 🧘",
    "Turn complex curl requests into repeatable health-check scripts. 🩺",
    "Alias your most frequent commands and fly through your daily workflow. 🦅",
    "Every Linux command you master unlocks a deeper layer of the machine. 🐧",
    "Keep your build pipelines fast, deterministic, and reproducible. 🏗️",
    "Use interactive rebase to craft a clean, readable commit history. ✨",
    "Inspect processes with `ps` and `top`—know what's running on your box! 🔍",
    "A single well-placed flag can transform a command's entire output. 🎯",
    "Shell history is a goldmine of solutions you've already figured out. 🪙",
    "Verify checksums, permissions, and paths before deploying. ✅",
    "Text streams are the universal interface of Unix engineering. 📜",
    "When the GUI slows you down, the terminal sets you free. 🪶",
    "Keep your dependencies lean and your build times snappy. ⚡",
    "Run the linter, run the tests, ship the binary! 🚀",
    "Great systems engineers feel right at home at the command prompt. 🏛️",
    "Use `watch` to monitor live changes without lifting a finger. 👀",
    "Batch-process files in seconds with a clean `for` loop in bash. 🔁",
    "Stay intentional with every command—measure twice, execute once. 📏",
    "Your terminal session is where ideas become running processes. 🔥",
    "Master SSH and remote debugging to work seamlessly from anywhere. 🛰️",
    "Every green check in the CLI is hard-earned engineering progress. 🏆",
    "Keep your shell tidy, your logs readable, and your focus sharp. 💎",
    "You're in command—keep driving this build to completion! 💪",
    "Pure CLI velocity: no distractions, just results. 🚀",
  ],

  browser: [
    "Research is formalized curiosity. It is poking and prying with a purpose. 🔍",
    "The capacity to learn is a gift; the ability to learn is a skill; the willingness to learn is a choice. 📚",
    "Knowledge is power. Information is liberating. 💡",
    "Stay curious, keep learning, never stop growing. 🌱",
    "Every expert was once a beginner who didn't give up. 🌟",
    "Learning never exhausts the mind. 🧠",
    "The more that you read, the more things you will know. 📖",
    "Invest in yourself. Your career is the engine of your wealth. 💎",
    "Great things never come from comfort zones. 🚀",
    "Focus on being productive instead of busy. 🎯",
    "Read the official documentation first—it's the ultimate source of truth! 📜",
    "Take what you just learned in this tab and test it in your editor now. 💻",
    "Close the tabs you've finished with—a clean browser is a clear mind! 🧹",
    "Deep research beats shallow skimming every single time. 🔬",
    "Bookmark the RFCs and deep dives that level up your architecture skills. 🏛️",
    "Don't get lost down the rabbit hole—keep your core task in sight! 🧭",
    "Great engineers read source code and release notes with equal curiosity. 🕵️",
    "Synthesize what you read into a three-line summary in your notes. 📝",
    "One high-quality technical article can reshape how you build systems. ⚡",
    "Check the benchmarks, read the caveats, understand the trade-offs. ⚖️",
    "Turn passive reading into active experimentation as fast as possible. 🧪",
    "When debugging via search, understand *why* the fix works before pasting! 🛠️",
    "Keep your browser focused: fewer open tabs, deeper comprehension. 🎯",
    "Explore how top open-source projects solve the exact problem you're facing. 🌐",
    "Learning the fundamentals makes every new framework ten times easier. 🧱",
    "Verify version numbers in docs so you're always reading the right API! ✅",
    "Use browser DevTools like a surgeon—inspect the DOM, network, and performance. 🩺",
    "A 10-minute deep dive into the spec saves two hours of trial and error. ⏱️",
    "Curiosity got you here; focused execution will get your feature shipped. 🚀",
    "Filter out the noise on the web and lock onto high-signal engineering resources. 📡",
    "Every tough bug has left clues in an issue tracker or changelog—find them! 🔦",
    "Study system design case studies to see how architectures behave at scale. 🏗️",
    "Don't just collect bookmarks—build something small with what you read today! 🌱",
    "Inspect the network tab: latency and payloads never lie. 📊",
    "When researching libraries, check maintenance health, types, and footprint. 📦",
    "Take a breath, close ten unused tabs, and refocus on your main goal. 🧘",
    "Reading great technical writing sharpens your own engineering communication. ✍️",
    "Use keyboard shortcuts in the browser to navigate docs at lightning speed. ⌨️",
    "Compare multiple sources before committing to a major architectural choice. 🔍",
    "Learn how the browser rendering pipeline works to build silky-smooth UIs. 🎨",
    "Security advisories and best practices keep your users safe—read them well. 🛡️",
    "Found the answer you were looking for? Jump back to your code and ship it! ⚡",
    "Intentional browsing turns the whole internet into your personal R&D lab. 🧪",
    "Understand the protocol beneath the abstraction—HTTP, WebSockets, and DNS. 🌐",
    "Great researchers know when to keep digging and when to start building. ⚖️",
    "Read the error code in the spec, not just the first forum post. 📖",
    "Your curiosity is fueling your next big engineering breakthrough. 💡",
    "Keep your research time-boxed so momentum stays on your side! ⏳",
    "Inspect CSS layout and computed styles directly—no more layout guesswork. 📐",
    "Every new concept you connect to existing knowledge sticks for life. 🧠",
    "Look for the simplest solution that satisfies your real constraints. 🎯",
    "Step away from infinite scroll and channel your energy into creation. 🔥",
    "Use DevTools profiling to turn sluggish pages into 60fps experiences. 🏎️",
    "Great builders are lifelong students of computer science and design. 🎓",
    "Capture that code example, adapt it cleanly, and write a test for it. ✅",
    "You've gathered the insights—now you're ready to execute with clarity! 🌟",
    "Smart research is the launchpad for effortless implementation. 🚀",
    "Stay purposeful in every tab you open—you're on a mission! 🧭",
    "Dive deep into the docs, extract the gold, and get back to flow. 🪙",
    "Knowledge compounds daily when you pair reading with hands-on building. 📈",
    "Check accessibility and responsiveness while inspecting your web UI. 🌍",
    "One clear mental model is worth a hundred bookmarked tutorials. 💎",
    "You're sharpening your edge with every focused research session. 🗡️",
    "Great discoveries happen when curiosity meets disciplined focus. 🔬",
    "Take the best idea from this page and turn it into working code today! 💪",
  ],

  general: [
    "Success is the sum of small efforts repeated day in and day out. 🌟",
    "Focus is a matter of deciding what things you're not going to do. 🎯",
    "The secret of getting ahead is getting started. 🚀",
    "Quality is not an act, it is a habit. ✨",
    "Do the hard jobs first. The easy jobs will take care of themselves. 💪",
    "Action is the foundational key to all success. 🔑",
    "Don't watch the clock; do what it does. Keep going. ⏰",
    "The way to get started is to quit talking and begin doing. 🎬",
    "Your limitation—it's only your imagination. 🌈",
    "Great things never came from comfort zones. 🔥",
    "Deep work is the superpower of the twenty-first century. 🧠",
    "One focused block of time can move mountains. 🏔️",
    "Protect your attention—it is your most valuable engineering asset. 🛡️",
    "Consistency beats intensity when intensity isn't consistent. 📈",
    "Small daily improvements compound into extraordinary mastery. 🌱",
    "Clear your desk, clear your mind, and lock into flow state. 🌊",
    "Finish what you start before opening the next task. 🏁",
    "Progress, not perfection, is what ships great products. 🚀",
    "Take pride in the details that even unseen code holds. 💎",
    "When energy dips, hydrate, stretch your shoulders, and reset. 💧",
    "Discipline is choosing between what you want now and what you want most. ⚖️",
    "Break the biggest goal on your list into a 15-minute first step. 🪜",
    "Calm, steady execution outpaces frantic rushing every time. 🧘",
    "You don't need more time; you need undivided focus on one priority. 🎯",
    "Every challenge you solve today upgrades your capabilities for tomorrow. ⚡",
    "Keep your promises to yourself—finish this milestone strong! 💪",
    "Creative breakthroughs reward those who stay with the problem longer. 💡",
    "Eliminate friction from your environment so focus feels natural. 🧰",
    "Celebrate small wins along the way—momentum loves recognition! 🏆",
    "Think clearly, act decisively, and iterate without ego. 🧭",
    "Your craft grows stronger with every deliberate hour you invest. 🛠️",
    "Breathe deeply, relax your jaw, and enjoy the process of building. 🍃",
    "High standards and kind self-discipline go hand in hand. 🌟",
    "Turn obstacles into data points and keep moving forward. 📊",
    "The best time to do deep work is right now. 🔥",
    "Keep your eyes on the horizon and your hands on the keyboard. ⌨️",
    "Simplicity in workflow creates space for brilliance in output. ✨",
    "Trust your preparation and lean into the hard problems. 🛡️",
    "One uninterrupted hour of creation is pure gold. 🪙",
    "Leave every project better than it was this morning. 🌱",
    "Energy flows where intention goes—direct yours wisely! 🎯",
    "Patience with the process, urgency with the action. ⚡",
    "Build things you're proud to put your name on. 🏛️",
    "Stay adaptable when plans change and steadfast in your goals. ⚓",
    "Rest deliberately when done so you can focus fiercely when working. 🌙",
    "Curiosity keeps your mind young; craftsmanship keeps your work timeless. 💎",
    "Don't let complexity intimidate you—untangle it one thread at a time. 🧶",
    "You are capable of solving harder problems than you think. 🧠",
    "Keep the main thing the main thing today. 🎯",
    "Momentum is built one completed task at a time. ✔️",
    "Show up with full presence and watch your productivity soar. 🦅",
    "Greatness is forged in quiet hours of focused execution. 🔥",
    "Keep building, keep learning, and enjoy the journey! 🚀",
    "Your focus right now is laying the foundation for big wins. 🏗️",
    "Stay locked in—you're doing fantastic work today! 🌟",
  ],
};

// Track recent quotes per category so quotes never repeat back-to-back
// or within the last N selections.
const recentQuotesByCategory: Record<QuoteCategory, string[]> = {
  vscode: [],
  antigravity: [],
  kiro: [],
  claude: [],
  chatgpt: [],
  konsole: [],
  terminal: [],
  browser: [],
  general: [],
};

const RECENT_HISTORY_LIMIT = 20;

export function getRandomQuote(category: QuoteCategory = "general"): string {
  const quotes = WORKFLOW_QUOTES[category] || WORKFLOW_QUOTES.general;
  const recent = recentQuotesByCategory[category] || [];

  // Filter out recently shown quotes so the user always sees a fresh quote
  const candidates = quotes.filter((q) => !recent.includes(q));
  const pool = candidates.length > 0 ? candidates : quotes;

  const picked = pool[Math.floor(Math.random() * pool.length)];

  recent.push(picked);
  if (recent.length > Math.min(RECENT_HISTORY_LIMIT, Math.floor(quotes.length / 2))) {
    recent.shift();
  }
  recentQuotesByCategory[category] = recent;

  return picked;
}

export function classifySessionCategory(resourceClass: string, title = ""): QuoteCategory {
  const cls = (resourceClass || "").trim().toLowerCase();
  const ttl = (title || "").trim().toLowerCase();
  const combined = `${cls} ${ttl}`;

  // 1. Check specific IDE / AI / Terminal classes first so window titles don't misclassify them
  if (cls.includes("antigravity")) return "antigravity";
  if (cls.includes("kiro")) return "kiro";
  if (cls.includes("chatgpt") || cls.includes("openai")) return "chatgpt";
  if (cls.includes("claude")) return "claude";
  if (cls.includes("konsole")) return "konsole";
  if (
    cls.includes("gnome-terminal") ||
    cls.includes("ptyxis") ||
    cls.includes("kgx") ||
    cls.includes("kitty") ||
    cls.includes("alacritty") ||
    cls.includes("wezterm") ||
    cls.includes("foot") ||
    cls.includes("tilix") ||
    cls.includes("terminator") ||
    cls.includes("xterm") ||
    cls.includes("yakuake") ||
    cls.includes("guake") ||
    cls.includes("warp") ||
    cls.includes("ghostty") ||
    cls.includes("terminal") ||
    cls.includes("bash") ||
    cls.includes("zsh") ||
    cls.includes("fish")
  ) {
    return "terminal";
  }
  if (cls.includes("code") || cls.includes("vscode") || cls.includes("cursor")) {
    return "vscode";
  }

  // 2. Check combined class + title for ChatGPT, Claude, Antigravity, Kiro, Konsole, Terminal, VS Code, Browser
  if (combined.includes("antigravity")) return "antigravity";
  if (combined.includes("kiro")) return "kiro";
  if (combined.includes("chatgpt") || combined.includes("openai")) return "chatgpt";
  if (combined.includes("claude")) return "claude";
  if (combined.includes("konsole")) return "konsole";
  if (
    combined.includes("visual studio code") ||
    combined.includes("vs code") ||
    combined.includes("vscode") ||
    combined.includes("cursor")
  ) {
    return "vscode";
  }
  if (
    combined.includes("terminal") ||
    combined.includes("gnome-terminal") ||
    combined.includes("ptyxis") ||
    combined.includes("kitty") ||
    combined.includes("alacritty") ||
    combined.includes("wezterm") ||
    combined.includes("bash") ||
    combined.includes("zsh") ||
    combined.includes("tmux")
  ) {
    return "terminal";
  }
  if (
    combined.includes("firefox") ||
    combined.includes("chrome") ||
    combined.includes("chromium") ||
    combined.includes("brave") ||
    combined.includes("edge") ||
    combined.includes("vivaldi") ||
    combined.includes("opera") ||
    combined.includes("zen") ||
    combined.includes("browser")
  ) {
    return "browser";
  }

  return "general";
}

export function getQuoteForWindow(resourceClass: string, title = ""): string {
  const category = classifySessionCategory(resourceClass, title);
  return getRandomQuote(category);
}
