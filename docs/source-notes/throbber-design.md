# Throbber / spinner

I have a design for a loading spinner/loading throbber.

Instead of showing something that is just moving, I'd like to show a little bit of what's going on inside the harness. Basically, what places in the code, or what operations, are currently happening that we are waiting on?

I think there are four things we could be waiting on:

1. Model response.
   This is the most common case. We've sent an API request and we're waiting for the model to respond. In fact, we can break this down into two pieces:

   * We've sent the API request, but we haven't yet received the start of the response. We're waiting for the response to begin.
   * The model is currently streaming. We've started receiving output and the response is in progress.

2. Tool calls.
   We run a command, read a file, process something large, etc. We're waiting for a tool call to execute.

3. Harness internals.
   For example:

   * Waiting for an MCP service to initialize.
   * Waiting for the results of a tool call to be processed.
   * The tool call is done, but we're doing some kind of internal reconciliation.
   * Reordering conversation history.
   * Any other internal compute or non-conversation-related task.

4. User input.
   In this case, I don't think we have a throbber. Instead, we probably have some kind of state where the throbber is not actually moving. Waiting on the user is either a different state entirely, or perhaps a very slow throbber.

I'm imagining maybe a little pentagon or square with small icons, where the icons flash or fade to indicate the current state. I don't know if that works.

Another idea is a small braille-style spinner with an icon next to it. Or maybe we have five different braille spinners, and a little pulse or "zip" moves between them. For example, a line connecting the states where a pulse travels along the border, or the border thickens as the pulse moves between the different braille spinners.

I'm not sure. We should probably explore a variety of designs here.

It needs to be compact, but it should also make the meaning of the different states obvious. Let's think through a number of possible approaches.
