import { writable } from 'svelte-local-storage-store';

export const authToken = writable<string | null>('auth-token', null);
