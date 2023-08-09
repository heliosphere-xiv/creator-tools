import base32Encode from 'base32-encode';
import base32Decode from 'base32-decode';
import { open } from '@tauri-apps/api/dialog';

export function uuidToBase32(input: string): string {
    const replaced = input.replaceAll('-', '');
    const bytes = hexToBytes(replaced);
    return base32Encode(bytes, 'Crockford').toLowerCase();
}

export function base32ToUuid(input: string): string {
    const bytes = base32Decode(input, 'Crockford');
    const array = Array.from(new Uint8Array(bytes));
    return bytesToHex(array);
}

function hexToBytes(hex: string): Uint8Array {
    const bytes = [];
    let c = 0;
    for (; c < hex.length; c += 2) {
        bytes.push(parseInt(hex.substring(c, c + 2), 16));
    }
    return Uint8Array.from(bytes);
}

function bytesToHex(bytes: Array<number>): string {
    const hex = [];
    let i = 0;
    for (; i < bytes.length; i++) {
        const current = bytes[i] < 0 ? bytes[i] + 256 : bytes[i];
        hex.push((current >>> 4).toString(16));
        hex.push((current & 0xF).toString(16));
    }
    return hex.join('').toLowerCase();
}

export function formatBytes(bytes: number, decimals = 2) {
    if (!+bytes) {
        return '0 bytes';
    }

    const k = 1024;
    const dm = decimals < 0 ? 0 : decimals;
    const sizes = ['bytes', 'KiB', 'MiB', 'GiB', 'TiB', 'PiB', 'EiB', 'ZiB', 'YiB'];

    const i = Math.floor(Math.log(bytes) / Math.log(k));
    const amt = parseFloat((bytes / Math.pow(k, i)).toFixed(dm));
    const formatted = new Intl.NumberFormat().format(amt);

    return `${formatted} ${sizes[i]}`;
}

export async function chooseMod(): Promise<string | null> {
    return await open({
        multiple: false,
        title: 'Choose Mod',
        filters: [
            // {
            //     name: 'Compatible mod packs',
            //     extensions: ['pmp', 'ttmp2', 'ttmp'],
            // },
            {
                name: 'Penumbra mod packs',
                extensions: ['pmp'],
            },
            // {
            //     name: 'TexTools mod packs',
            //     extensions: ['ttmp2', 'ttmp'],
            // },
        ],
    }) as string | null;
}
