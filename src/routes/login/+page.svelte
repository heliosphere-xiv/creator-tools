<script lang='ts'>
    import { GQL_LogIn } from '$houdini';
    import { authToken } from '$lib/stores';
    import { goto } from '$app/navigation';

    if ($authToken) {
        goto('/');
    }

    let processing = false;
    let error: string | null = null;

    let username = '';
    let password = '';

    async function submit() {
        processing = true;
        error = null;

        try {
            await inner();
        } catch (e) {
            console.log(e);
            error = e.toString();
        } finally {
            processing = false;
        }
    }

    async function inner() {
        if (!username || !password) {
            return;
        }

        const resp = await GQL_LogIn.mutate({
            username,
            password,
        });

        const token = resp?.login?.token;
        authToken.set(token || null);

        if (token) {
            await goto('/');
        }
    }
</script>

<article>
    <header>
        <h1>Log in</h1>
    </header>

    <p>
        Certain features of this app require you to be logged in. You can do so here!
    </p>
</article>

{#if error}
    <p>An error occurred. Please try again.</p>
{/if}

<form on:submit|preventDefault={submit}>
    <label>
        Username
        <input type='text' required placeholder='Username' bind:value={username} />
    </label>

    <label>
        Password
        <input type='password' required placeholder='Password' bind:value={password} />
    </label>

    <button type='submit' aria-busy={processing} disabled={processing}>Log in</button>
</form>
