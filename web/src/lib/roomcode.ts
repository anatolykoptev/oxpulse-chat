/**
 * Generate a random room code in format XXXX-0000
 * 4 uppercase letters + dash + 4 digits = ~4.5 billion combinations
 */
export function generateRoomCode(): string {
  const letters = 'ABCDEFGHJKLMNPQRSTUVWXYZ'; // no I, O (confusing with 1, 0)
  const digits = '0123456789';
  let code = '';
  for (let i = 0; i < 4; i++) code += letters[Math.floor(Math.random() * letters.length)];
  code += '-';
  for (let i = 0; i < 4; i++) code += digits[Math.floor(Math.random() * digits.length)];
  return code;
}

/** Check if string is a valid room code (XXXX-0000) or UUID */
export function isValidRoomId(str: string): boolean {
  const codeRegex = /^[A-Z]{4}-[0-9]{4}$/;
  const uuidRegex = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
  return codeRegex.test(str) || uuidRegex.test(str);
}

/** Format input as room code: uppercase, auto-insert dash after 4 chars */
export function formatRoomInput(raw: string): string {
  let clean = raw.toUpperCase().replace(/[^A-Z0-9]/g, '');
  if (clean.length > 4) {
    clean = clean.slice(0, 4) + '-' + clean.slice(4, 8);
  }
  return clean.slice(0, 9); // max XXXX-0000
}
