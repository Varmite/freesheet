'use strict';
// Checking a "Sign in with Google" ID token.
//
// The browser gets a signed token (a JWT) from Google Identity Services and posts it to
// /auth/google. google-auth-library checks Google's signature, that it was issued for
// our GOOGLE_CLIENT_ID (audience), by Google (issuer) and has not expired. We also
// insist the email address is verified, and, if ALLOWED_EMAILS is set, on the list.
//
// TEST_MODE=1 (tests only, never on the real server) swaps Google for a stand-in that
// accepts tokens signed with TEST_SECRET, so tests can sign in as made-up people and
// check that forged, expired or wrong-audience tokens are still refused.

const crypto = require('crypto');

class AuthError extends Error {}

function checkClaims(p, cfg) {
  if (!p || !p.sub) throw new AuthError('The sign-in token has no account id.');
  if (p.email_verified !== true && p.email_verified !== 'true') throw new AuthError('Your Google email address is not verified.');
  const email = String(p.email || '').toLowerCase();
  if (cfg.allowedEmails.length && !cfg.allowedEmails.includes(email)) {
    throw new AuthError(`${email || 'This account'} is not allowed to use this FreeSheet server.`);
  }
  return { sub: String(p.sub), email, name: String(p.name || '') };
}

function googleVerifier(cfg) {
  const { OAuth2Client } = require('google-auth-library');
  const client = new OAuth2Client(cfg.googleClientId);
  return async (idToken) => {
    if (!cfg.googleClientId) throw new AuthError('GOOGLE_CLIENT_ID is not set on the server.');
    let ticket;
    try {
      ticket = await client.verifyIdToken({ idToken, audience: cfg.googleClientId });
    } catch (e) {
      throw new AuthError(`Google sign-in could not be checked: ${e.message}`);
    }
    return checkClaims(ticket.getPayload(), cfg);
  };
}

// ---- the test stand-in: header.payload.signature like a JWT, but HMAC-SHA256.
const b64 = (s) => Buffer.from(s).toString('base64url');
function signTestToken(claims, secret) {
  const body = `${b64(JSON.stringify({ alg: 'HS256', typ: 'JWT' }))}.${b64(JSON.stringify(claims))}`;
  return `${body}.${crypto.createHmac('sha256', secret).update(body).digest('base64url')}`;
}
function testVerifier(cfg) {
  return async (idToken) => {
    const parts = String(idToken).split('.');
    if (parts.length !== 3) throw new AuthError('Malformed token.');
    const expect = crypto.createHmac('sha256', cfg.testSecret).update(`${parts[0]}.${parts[1]}`).digest();
    const got = Buffer.from(parts[2], 'base64url');
    if (got.length !== expect.length || !crypto.timingSafeEqual(got, expect)) throw new AuthError('Bad signature.');
    let p;
    try { p = JSON.parse(Buffer.from(parts[1], 'base64url').toString('utf8')); } catch { throw new AuthError('Malformed token.'); }
    if (p.aud !== cfg.googleClientId) throw new AuthError('Token is for another app.');
    if (p.iss !== 'https://accounts.google.com' && p.iss !== 'accounts.google.com') throw new AuthError('Token not issued by Google.');
    if (!(Number(p.exp) * 1000 > Date.now())) throw new AuthError('Token has expired.');
    return checkClaims(p, cfg);
  };
}

function createVerifier(cfg) {
  return cfg.testMode ? testVerifier(cfg) : googleVerifier(cfg);
}

module.exports = { createVerifier, signTestToken, AuthError };
