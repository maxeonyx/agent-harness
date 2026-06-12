# User turn

This is a cool rethinking of what it means to collaborate with an agent.

Instead of me "sending messages" and asking the agent to show me stuff, I want an *inline* experience where I can open up CLI tools (or UI tools in a GUI harness) and view, edit files, or a terminal tool where I can run commands in, and the outputs get attached to the conversation same as if an agent used a tool.

This way I can actually work too! And the agent gets to see what I've done. I don't have to worry about the overhead of telling the agent that I made "out of band" changes.

I just go and do them. I just run the commands. I just edit the AGENTS.md (and the harness knows & can update it & know that next handover / next new session it should use the new content)

What other tools should there be?

- file tool (opens a file explorer, selecting a file opens an interactive editor. tracks changes as diff, and importantly - tracks what the user looked at, including the file explorer. tracks if the user did a "find" command etc. should open files in fully collapsed view to start with.)
- terminal tool. run command in terminal. terminal should be persistent rather than ephemeral, probably. but quite keen on it being not an *actual* bash / fish terminal, as it would be ideal to be able to fork / undo it with the message history. This is totally a stretch goal though. Likewise, REPL tool too that works the same way.
- search tool. search for stuff. show the history of what the user searched for and show what they looked for and found.

The way we should think about each tool is that it has two outputs:
- UI & actions for the user 
- A transcript (written live, eg. when the user opens a file, and is still looking at it, the agent might get to know this to aid collaboration) which shows the agent what the user is doing, framed clearly as "useful context on what the user is doing". Just as the user has a view of the agent's tool calls which is clear to the user "this is what the agent is/was doing" so does the agent now have the reverse. Also a corollory is that agent tools should (and do already - but why not make this more explicit!) provide UI implementations (for both web & TUI). The tools own both projections - so the user tool owns both the UI and the context compression / projection. Same for agent tools.

The point is that the user and the agent should be on the same page about the history, not that they should see exactly the same stuff.

Ideally we might also record/transcribe the user's voice at the same time, so they can talk while they work and have that attached too.

When the agent picks up the task, it should ideally have all the context it needs.

There would be two "UIs" for the same states for the user tools, though. The view that the user sees (file explorer, file content rendered, etc) and the context that gets attached (which should have all of the same important information, including what the user saw but *didn't* use, but can exclude any purely visual or irrelevant info, or can exclude intermediate states that the user doesn't need.

The main harness view should stay a chat UI, but then a hotkey would put the user into the tool - eg. $ would immediately open the terminal view ready to type a command (esc to return to the harness view would leave the $ in the terminal so that the user can actually type $ normally). I think @ for file. Not sure what for search.

In this harness, pressing enter would not automatically send the message. it would be ctrl+enter. Not sure whether there should be a distinction between enter (send message?) and shift+enter (new line in existing message). Doesn't seem very important, as ultimately we're sending one message part, I think? Although maybe we send multiple user message parts to the model. I guess it probably sees them differently, so it probably makes sense to keep the distinction. Yeah - we keep the distinciton. And user tool calls are multiple message parts.

We need to support as many tools as needed to allow the user to fully make decisions "in band". for example, we probably want to have a github tool for the user - an interactive gh terminal to view PR desc, comments, reviews, diff etc. ideally we just integrate (BUT need to track what's going on inside, so may need to fork - this goes for other tools too) an existing tool for this.

All of the information that the user used to make their decision should ideally remain available to the agent, albeit in as minimal a form as possible, NOT just the decision outcome.

The user tools are NOT the same as the agent tools, and we should make sure that the context is clear to the agent that it has a different tool set to the user (we don't want the agent trying to use the user's tools).

This concept is NOT just for a TUI - it's for a harness which could later also be a GUI. We build GUI/web support in from the start, even if unimplemented. This would be more natural in fact as the user may commonly want to use a web browser.

Oh - one obvious user tool is a subagent tool!! e.g "find me that nix issue where XYZ" and then the user's prompt + the subagent's response is included. Yeah, that's really great. Support both forked & fresh agents. Forked should warn if cache likely expired (note that forked can still be cheaper even if cache expired, if the model would have to do many sequential tool calls to get back up to speed. User can judge.). In this case we don't have to attach what the subagent saw - only what the user saw.

user turn presents concurrency issues with the model. I don't think we have to think too hard about this, but we might reject an agent's updates to a file if the user currently has it open, or has edited it since the user last did so. I don't think we should be too eager about that. Maybe only if the updates actually conflict. We don't want to overprescribe live collaboration. The fact that the agent gets to observe the user is already a massive win.

