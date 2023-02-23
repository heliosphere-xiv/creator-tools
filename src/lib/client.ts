import { HoudiniClient } from '$houdini';
import { get } from 'svelte/store';
import { authToken } from '$lib/stores';
import { Body, fetch } from '@tauri-apps/api/http';
import { fetch as houdiniFetchPlugin } from '$houdini/plugins';
import type { RequestHandlerArgs } from 'houdini/build/runtime/client/plugins';

async function fetchQuery(args: RequestHandlerArgs) {
    const {
        text = '',
        variables = {},
    } = args;

    const headers: Record<string, string> = {
        'Content-Type': 'application/json',
        'User-Agent': 'hs-creator-tools/1.2.0',
    };

    const auth = get(authToken);
    if (auth) {
        headers['x-auth-token'] = auth;
    }

    const resp = await fetch('https://heliosphere.app/api/api/graphql', {
        method: 'POST',
        headers,
        body: Body.json({
            query: text,
            variables,
        }),
    });

    return resp.data as any;
}

export default new HoudiniClient({
    url: 'https://heliosphere.app/api/api/graphql',
    plugins: [houdiniFetchPlugin(fetchQuery)],
});
