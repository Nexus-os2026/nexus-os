//! Supabase Client Setup — generates client initialization and env files.

use super::GeneratedFile;

/// Generate Supabase client setup files.
///
/// Returns: supabase.ts + .env.local.example
pub fn generate_supabase_client() -> Vec<GeneratedFile> {
    vec![generate_client_file(), generate_env_example()]
}

fn generate_client_file() -> GeneratedFile {
    GeneratedFile {
        path: "src/lib/supabase.ts".into(),
        content: r#"import { createClient } from '@supabase/supabase-js'
import type { Database } from '../types/database'

const supabaseUrl = import.meta.env.VITE_SUPABASE_URL
const supabaseAnonKey = import.meta.env.VITE_SUPABASE_ANON_KEY

if (!supabaseUrl || !supabaseAnonKey) {
  throw new Error(
    'Missing Supabase environment variables. ' +
    'Copy .env.local.example to .env.local and fill in your Supabase project credentials.'
  )
}

export const supabase = createClient<Database>(supabaseUrl, supabaseAnonKey)
"#
        .into(),
    }
}

fn generate_env_example() -> GeneratedFile {
    GeneratedFile {
        path: ".env.local.example".into(),
        content: r#"# Supabase connection — get these from your Supabase project settings → API
VITE_SUPABASE_URL=https://your-project.supabase.co
VITE_SUPABASE_ANON_KEY=your-anon-key-here
"#
        .into(),
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generates_client_file() {
        let files = generate_supabase_client();
        let client = files.iter().find(|f| f.path == "src/lib/supabase.ts");
        assert!(client.is_some());
        let content = &client.unwrap().content;
        assert!(content.contains("createClient"));
        assert!(content.contains("VITE_SUPABASE_URL"));
        assert!(content.contains("VITE_SUPABASE_ANON_KEY"));
        assert!(content.contains("Database"));
    }

    #[test]
    fn test_generates_env_example() {
        let files = generate_supabase_client();
        let env = files.iter().find(|f| f.path == ".env.local.example");
        assert!(env.is_some());
        let content = &env.unwrap().content;
        assert!(content.contains("VITE_SUPABASE_URL"));
        assert!(content.contains("VITE_SUPABASE_ANON_KEY"));
    }
}
