# process

we'll implement spikes of various bits, *explicitly* marked as experiments, labelled so, under a separate part of the repo. that's for getting the design right, and when we move to a different aspect we do that again. We integrate into core only afterwards, and we do so cleanly, perfectly and minimally - when we integrate into core we leave NOTHING half done.

Each slice/spike should exercise the thesis of that slice. The good thing about these spikes is that they *don't* have to fit into the whole - so we can eg. go without evented model, or go without multiple UIs, or go without plugins, etc. in these spikes.

We set up the repo first. development process before product, always. we set up checks like linting, formatting, version checks, CI checks etc, before we start implementing. We do our best to do things right the first time, and when it inevitably goes a bit wrong, we improve it for next time.

The point of the process is getting things right, tested, and cleanly integrated - not the process itself.
