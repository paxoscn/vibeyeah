-- Historical example seed for the `organizations` table.
--
-- Deprecated: on a fresh database the backend no longer needs a hand-seeded
-- row. Its first-run wizard creates the initial organization and binds the
-- Feishu bot (or you bind the bot later via the Feishu Open Platform). This
-- file is kept only as a reference for the table shape.
--
-- If you prefer to seed the row yourself (e.g. non-interactive deployments),
-- the backend still picks it up as long as the `organizations` table is
-- non-empty. Replace every placeholder, and never commit real credentials.
--
--   k8s_kubeconfig   : base64 of a kubeconfig for the cluster, or NULL to use
--                      the backend's in-cluster / default client.
--   lark_app_id      : your Feishu/Lark app id        (format: cli_xxxxxxxxxxxxxxxx)
--   lark_app_secret  : your Feishu/Lark app secret
--   image_pull_secret: name of the K8s imagePullSecret in the namespace
INSERT INTO organizations (
    id, name, slug, k8s_namespace, k8s_kubeconfig,
    lark_tenant_key, lark_app_id, lark_app_secret, image_pull_secret, created_at, updated_at)
VALUES (
  '00000000-0000-0000-0000-000000000000',
  'default',
  'default',
  'vibeyeah',
  NULL,
  '',
  'cli_xxxxxxxxxxxxxxxx',
  'REPLACE_WITH_YOUR_LARK_APP_SECRET',
  'your-image-pull-secret',
  NOW(),
  NOW()
);
