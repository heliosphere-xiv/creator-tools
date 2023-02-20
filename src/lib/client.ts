import { HoudiniClient, type RequestHandlerArgs } from '$houdini';
import { Body, fetch } from '@tauri-apps/api/http';
import { get } from 'svelte/store';
import { authToken } from '$lib/stores';

async function fetchQuery(args: RequestHandlerArgs) {
    const {
        text = '',
        variables = {},
    } = args;

    const headers: Record<string, string> = {
        'Content-Type': 'application/json',
        'User-Agent': 'hs-creator-tools/1.1.4',
    };

    const auth = get(authToken);
    if (auth) {
        headers['x-auth-token'] = auth;
    }

    const url = 'https://heliosphere.app/api/api/graphql';
    const result = await fetch(url, {
        method: 'POST',
        headers,
        body: Body.json({
            query: text,
            variables,
        }),
    });

    return result.data as any;
}

export default new HoudiniClient(fetchQuery);
