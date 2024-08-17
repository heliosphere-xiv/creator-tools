import { HoudiniClient } from '$houdini';
import { Body, fetch } from '@tauri-apps/api/http';
import { fetch as houdiniFetchPlugin } from '$houdini/plugins';
import type { RequestHandlerArgs } from 'houdini/build/runtime/client/plugins';

const URL = 'https://heliosphere.app/api/api/graphql';
// const URL = 'http://localhost:42011/api/graphql';

async function fetchQuery(args: RequestHandlerArgs) {
    const {
        text = '',
        variables = {},
    } = args;

    const headers: Record<string, string> = {
        'Content-Type': 'application/json',
        'User-Agent': 'hs-creator-tools/2.0.5',
    };

    const resp = await fetch(URL, {
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
    url: URL,
    plugins: [houdiniFetchPlugin(fetchQuery)],
});
