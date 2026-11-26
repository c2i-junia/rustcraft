import type { Me } from './api';

/** The admin who is logged in, once GET /me has answered. */
export const session = $state<{ me: Me | null }>({ me: null });
