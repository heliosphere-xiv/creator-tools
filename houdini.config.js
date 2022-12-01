/// <references types="houdini-svelte">

/** @type {import('houdini').ConfigFile} */
const config = {
    apiUrl: "http://127.0.0.1:42011/api/graphql",
    plugins: {
        "houdini-svelte": {
            "client": "./src/lib/client"
        }
    },
    scalars: {
        UUID: {
            type: 'string',
        },
        DateTime: {
            type: 'string',
        },
        JSON: {
            type: 'any',
        },
        JSONObject: {
            type: 'any',
        },
        FileList: {
            type: 'Record<string, [string | null, string | null, string][]>',
        }
    },
}

export default config
