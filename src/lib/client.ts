import { HoudiniClient } from '$houdini';
import { get } from 'svelte/store';
import { authToken } from '$lib/stores';

function authHeader(): { 'x-auth-token': string } | {} {
    const auth = get(authToken);
    if (auth) {
        return {
            'x-auth-token': auth,
        };
    }

    return {};
}

export default new HoudiniClient({
    url: 'https://heliosphere.app/api/api/graphql',
    fetchParams({ session }) {
        return {
            headers: {
                'Content-Type': 'application/json',
                'User-Agent': 'hs-creator-tools/1.2.0',
                ...authHeader(),
            },
        };
    },
});
