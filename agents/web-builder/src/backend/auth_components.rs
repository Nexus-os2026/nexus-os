//! Auth Component Generation — deterministic React auth scaffolding.
//!
//! Generates: AuthProvider, LoginForm, SignUpForm, AuthGuard, UserMenu.
//! All components use token-based Tailwind classes (no hardcoded colors).

use super::GeneratedFile;

/// Generate all auth components for a Supabase-connected React app.
pub fn generate_auth_components() -> Vec<GeneratedFile> {
    vec![
        generate_auth_provider(),
        generate_login_form(),
        generate_signup_form(),
        generate_auth_guard(),
        generate_user_menu(),
        generate_use_auth_hook(),
    ]
}

fn generate_auth_provider() -> GeneratedFile {
    GeneratedFile {
        path: "src/components/auth/AuthProvider.tsx".into(),
        content: r#"import { createContext, useContext, useEffect, useState, type ReactNode } from 'react'
import { supabase } from '../../lib/supabase'
import type { User, Session } from '@supabase/supabase-js'

interface AuthContextType {
  user: User | null
  session: Session | null
  loading: boolean
  signOut: () => Promise<void>
}

const AuthContext = createContext<AuthContextType>({
  user: null,
  session: null,
  loading: true,
  signOut: async () => {},
})

export function AuthProvider({ children }: { children: ReactNode }) {
  const [user, setUser] = useState<User | null>(null)
  const [session, setSession] = useState<Session | null>(null)
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    supabase.auth.getSession().then(({ data: { session: s } }) => {
      setSession(s)
      setUser(s?.user ?? null)
      setLoading(false)
    })

    const { data: { subscription } } = supabase.auth.onAuthStateChange((_event, s) => {
      setSession(s)
      setUser(s?.user ?? null)
      setLoading(false)
    })

    return () => subscription.unsubscribe()
  }, [])

  const signOut = async () => {
    await supabase.auth.signOut()
    setUser(null)
    setSession(null)
  }

  return (
    <AuthContext.Provider value={{ user, session, loading, signOut }}>
      {children}
    </AuthContext.Provider>
  )
}

export function useAuthContext() {
  return useContext(AuthContext)
}

export default AuthProvider
"#
        .into(),
    }
}

fn generate_login_form() -> GeneratedFile {
    GeneratedFile {
        path: "src/components/auth/LoginForm.tsx".into(),
        content: r#"import { useState, type FormEvent } from 'react'
import { supabase } from '../../lib/supabase'

interface LoginFormProps {
  onSuccess?: () => void
  onToggleSignUp?: () => void
}

export default function LoginForm({ onSuccess, onToggleSignUp }: LoginFormProps) {
  const [email, setEmail] = useState('')
  const [password, setPassword] = useState('')
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault()
    setError(null)
    setLoading(true)

    const { error: err } = await supabase.auth.signInWithPassword({ email, password })

    if (err) {
      setError(err.message)
      setLoading(false)
    } else {
      onSuccess?.()
    }
  }

  return (
    <div className="w-full max-w-sm mx-auto">
      <h2 className="text-2xl font-heading font-bold text-text-primary mb-lg">Sign In</h2>
      <form onSubmit={handleSubmit} className="space-y-md">
        <div>
          <label className="block text-sm font-medium text-text-secondary mb-xs">Email</label>
          <input
            type="email"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            required
            className="w-full px-md py-sm bg-bg-secondary border border-border rounded-md text-text-primary focus:outline-none focus:ring-2 focus:ring-primary"
            placeholder="you@example.com"
          />
        </div>
        <div>
          <label className="block text-sm font-medium text-text-secondary mb-xs">Password</label>
          <input
            type="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            required
            className="w-full px-md py-sm bg-bg-secondary border border-border rounded-md text-text-primary focus:outline-none focus:ring-2 focus:ring-primary"
            placeholder="••••••••"
          />
        </div>
        {error && (
          <p className="text-sm text-red-500">{error}</p>
        )}
        <button
          type="submit"
          disabled={loading}
          className="w-full bg-btn-bg text-btn-text px-lg py-sm rounded-md font-semibold transition-colors duration-fast hover:opacity-90 disabled:opacity-50"
        >
          {loading ? 'Signing in...' : 'Sign In'}
        </button>
      </form>
      {onToggleSignUp && (
        <p className="mt-md text-sm text-text-secondary text-center">
          Don't have an account?{' '}
          <button onClick={onToggleSignUp} className="text-primary hover:underline">
            Sign Up
          </button>
        </p>
      )}
    </div>
  )
}
"#
        .into(),
    }
}

fn generate_signup_form() -> GeneratedFile {
    GeneratedFile {
        path: "src/components/auth/SignUpForm.tsx".into(),
        content: r#"import { useState, type FormEvent } from 'react'
import { supabase } from '../../lib/supabase'

interface SignUpFormProps {
  onSuccess?: () => void
  onToggleLogin?: () => void
}

export default function SignUpForm({ onSuccess, onToggleLogin }: SignUpFormProps) {
  const [email, setEmail] = useState('')
  const [password, setPassword] = useState('')
  const [confirmPassword, setConfirmPassword] = useState('')
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault()
    setError(null)

    if (password !== confirmPassword) {
      setError('Passwords do not match')
      return
    }
    if (password.length < 6) {
      setError('Password must be at least 6 characters')
      return
    }

    setLoading(true)
    const { error: err } = await supabase.auth.signUp({ email, password })

    if (err) {
      setError(err.message)
      setLoading(false)
    } else {
      onSuccess?.()
    }
  }

  return (
    <div className="w-full max-w-sm mx-auto">
      <h2 className="text-2xl font-heading font-bold text-text-primary mb-lg">Create Account</h2>
      <form onSubmit={handleSubmit} className="space-y-md">
        <div>
          <label className="block text-sm font-medium text-text-secondary mb-xs">Email</label>
          <input
            type="email"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            required
            className="w-full px-md py-sm bg-bg-secondary border border-border rounded-md text-text-primary focus:outline-none focus:ring-2 focus:ring-primary"
            placeholder="you@example.com"
          />
        </div>
        <div>
          <label className="block text-sm font-medium text-text-secondary mb-xs">Password</label>
          <input
            type="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            required
            className="w-full px-md py-sm bg-bg-secondary border border-border rounded-md text-text-primary focus:outline-none focus:ring-2 focus:ring-primary"
            placeholder="••••••••"
          />
        </div>
        <div>
          <label className="block text-sm font-medium text-text-secondary mb-xs">Confirm Password</label>
          <input
            type="password"
            value={confirmPassword}
            onChange={(e) => setConfirmPassword(e.target.value)}
            required
            className="w-full px-md py-sm bg-bg-secondary border border-border rounded-md text-text-primary focus:outline-none focus:ring-2 focus:ring-primary"
            placeholder="••••••••"
          />
        </div>
        {error && (
          <p className="text-sm text-red-500">{error}</p>
        )}
        <button
          type="submit"
          disabled={loading}
          className="w-full bg-btn-bg text-btn-text px-lg py-sm rounded-md font-semibold transition-colors duration-fast hover:opacity-90 disabled:opacity-50"
        >
          {loading ? 'Creating account...' : 'Sign Up'}
        </button>
      </form>
      {onToggleLogin && (
        <p className="mt-md text-sm text-text-secondary text-center">
          Already have an account?{' '}
          <button onClick={onToggleLogin} className="text-primary hover:underline">
            Sign In
          </button>
        </p>
      )}
    </div>
  )
}
"#
        .into(),
    }
}

fn generate_auth_guard() -> GeneratedFile {
    GeneratedFile {
        path: "src/components/auth/AuthGuard.tsx".into(),
        content: r#"import { type ReactNode } from 'react'
import { useAuthContext } from './AuthProvider'
import LoginForm from './LoginForm'

interface AuthGuardProps {
  children: ReactNode
  fallback?: ReactNode
}

export default function AuthGuard({ children, fallback }: AuthGuardProps) {
  const { user, loading } = useAuthContext()

  if (loading) {
    return (
      <div className="flex items-center justify-center min-h-screen bg-bg">
        <div className="text-text-secondary text-sm">Loading...</div>
      </div>
    )
  }

  if (!user) {
    return fallback ? (
      <>{fallback}</>
    ) : (
      <div className="flex items-center justify-center min-h-screen bg-bg p-lg">
        <LoginForm />
      </div>
    )
  }

  return <>{children}</>
}
"#
        .into(),
    }
}

fn generate_user_menu() -> GeneratedFile {
    GeneratedFile {
        path: "src/components/auth/UserMenu.tsx".into(),
        content: r#"import { useState } from 'react'
import { useAuthContext } from './AuthProvider'

export default function UserMenu() {
  const { user, signOut } = useAuthContext()
  const [open, setOpen] = useState(false)

  if (!user) return null

  const initials = (user.email ?? '?')[0].toUpperCase()

  return (
    <div className="relative">
      <button
        onClick={() => setOpen(!open)}
        className="w-8 h-8 rounded-full bg-primary text-btn-text flex items-center justify-center text-sm font-semibold hover:opacity-90 transition-opacity duration-fast"
        aria-label="User menu"
      >
        {initials}
      </button>
      {open && (
        <div className="absolute right-0 mt-xs w-48 bg-card-bg border border-card-border rounded-md shadow-lg z-50">
          <div className="px-md py-sm border-b border-border">
            <p className="text-xs text-text-secondary truncate">{user.email}</p>
          </div>
          <button
            onClick={async () => { await signOut(); setOpen(false); }}
            className="w-full text-left px-md py-sm text-sm text-text-primary hover:bg-bg-secondary transition-colors duration-fast"
          >
            Sign Out
          </button>
        </div>
      )}
    </div>
  )
}
"#
        .into(),
    }
}

fn generate_use_auth_hook() -> GeneratedFile {
    GeneratedFile {
        path: "src/hooks/useAuth.ts".into(),
        content: r#"export { useAuthContext as useAuth } from '../components/auth/AuthProvider'
"#
        .into(),
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generates_auth_provider() {
        let files = generate_auth_components();
        let provider = files.iter().find(|f| f.path.contains("AuthProvider"));
        assert!(provider.is_some(), "should generate AuthProvider");
        let content = &provider.unwrap().content;
        assert!(content.contains("supabase"), "should import supabase");
        assert!(
            content.contains("onAuthStateChange"),
            "should listen for auth changes"
        );
    }

    #[test]
    fn test_generates_login_form() {
        let files = generate_auth_components();
        let login = files.iter().find(|f| f.path.contains("LoginForm"));
        assert!(login.is_some(), "should generate LoginForm");
        let content = &login.unwrap().content;
        assert!(
            content.contains("signInWithPassword"),
            "should call signInWithPassword"
        );
    }

    #[test]
    fn test_generates_signup_form() {
        let files = generate_auth_components();
        let signup = files.iter().find(|f| f.path.contains("SignUpForm"));
        assert!(signup.is_some(), "should generate SignUpForm");
        let content = &signup.unwrap().content;
        assert!(content.contains("signUp"), "should call signUp");
    }

    #[test]
    fn test_generates_auth_guard() {
        let files = generate_auth_components();
        let guard = files.iter().find(|f| f.path.contains("AuthGuard"));
        assert!(guard.is_some(), "should generate AuthGuard");
        let content = &guard.unwrap().content;
        assert!(
            content.contains("useAuthContext"),
            "should check auth context"
        );
    }

    #[test]
    fn test_components_use_tailwind_tokens() {
        let files = generate_auth_components();
        for f in &files {
            // Skip context providers and hooks — they have no UI
            if f.path.contains("AuthProvider") || f.path.ends_with(".ts") {
                continue;
            }
            if f.path.ends_with(".tsx") {
                // Should use token classes, not hardcoded colors
                assert!(
                    !f.content.contains("bg-blue-") && !f.content.contains("bg-gray-"),
                    "{} should not have hardcoded Tailwind colors",
                    f.path
                );
                // Should use semantic token classes
                let has_tokens = f.content.contains("bg-btn-bg")
                    || f.content.contains("text-text-primary")
                    || f.content.contains("bg-bg")
                    || f.content.contains("text-primary");
                assert!(
                    has_tokens,
                    "{} should use token-based Tailwind classes",
                    f.path
                );
            }
        }
    }

    #[test]
    fn test_generates_use_auth_hook() {
        let files = generate_auth_components();
        let hook = files.iter().find(|f| f.path.contains("useAuth"));
        assert!(hook.is_some(), "should generate useAuth hook");
    }
}
