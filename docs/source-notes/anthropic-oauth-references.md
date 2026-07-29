# Required to get claude sub working with other harnesses

https://github.com/nitishxyz/otto/blob/main/packages/sdk/src/providers/src/anthropic-oauth-client.ts

https://github.com/leohenon/pi-anthropic-oauth

The above seems solid but also I feel this comes from a stricter time and we'd do well to somehow figure out if loosening it is ok

https://github.com/griffinmartin/opencode-claude-auth

Note: nothing about Claude Team sub mentioned. should work the same hopefully.

Oh-My-Pi apparently figured it out but this needs sorting through, I couldn't find the exact bits.

https://github.com/can1357/oh-my-pi/blob/main/packages/ai/src/registry/anthropic.ts
https://github.com/can1357/oh-my-pi/blob/main/packages/ai/src/registry/oauth/anthropic.ts
https://github.com/can1357/oh-my-pi/blob/main/packages/ai/src/utils/anthropic-auth.ts
https://github.com/can1357/oh-my-pi/blob/main/packages/ai/src/providers/anthropic-client.ts
https://github.com/can1357/oh-my-pi/blob/main/packages/ai/src/providers/anthropic-wire.ts
https://github.com/can1357/oh-my-pi/blob/main/packages/ai/src/providers/anthropic-messages-server.ts
https://github.com/can1357/oh-my-pi/blob/main/packages/ai/src/providers/anthropic-messages-server-schema.ts
https://github.com/can1357/oh-my-pi/blob/main/packages/ai/src/providers/openai-anthropic-shim.ts
https://github.com/can1357/oh-my-pi/blob/main/packages/ai/src/registry/api-key-validation.ts
https://github.com/can1357/oh-my-pi/blob/main/packages/ai/src/error/classes.ts
https://github.com/can1357/oh-my-pi/blob/main/packages/ai/src/utils/tool-choice.ts
https://github.com/can1357/oh-my-pi/blob/main/packages/catalog/src/compat/anthropic.ts
https://github.com/can1357/oh-my-pi/blob/main/packages/catalog/src/identity/classify.ts
https://github.com/can1357/oh-my-pi/blob/main/docs/toolconv/anthropic.md
https://github.com/can1357/oh-my-pi/blob/main/docs/providers.md
https://github.com/can1357/oh-my-pi/blob/main/packages/snapcompact/research/anthropic_api.py
https://github.com/can1357/oh-my-pi/blob/main/packages/snapcompact/research/providers.py


this seems really desperate way to do it:

https://gist.github.com/synistr/906f86f0139f0bf3771b0e7127fe31aa
which uses
https://github.com/router-for-me/CLIProxyAPI
*shudder*

this one's not open source but might be worth downloading and inspecting:

https://www.npmjs.com/package/@zgltyq/pi-provider-claude

another: https://github.com/cortexkit/anthropic-auth
and another: https://github.com/leohenon/op-anthropic-auth (might want to check this author for others)

let's collect up all the tricks.

I also ideally want a timeline of tricks added to these implementations to see how active anthropic has been on their oauth / third party harness restrictions.

