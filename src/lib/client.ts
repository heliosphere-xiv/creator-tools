import { HoudiniClient, type RequestHandlerArgs } from '$houdini';
import { fetch, Body } from '@tauri-apps/api/http';

async function fetchQuery({
	text = '',
	variables = {},
	metadata
}: RequestHandlerArgs) {
	const url = 'https://heliosphere.app/api/api/graphql';
	const result = await fetch(url, {
		method: 'POST',
		headers: {
			'Content-Type': 'application/json'
		},
		body: Body.json({
			query: text,
			variables
		}),
	});

	return result.data as any;
}

export default new HoudiniClient(fetchQuery);
