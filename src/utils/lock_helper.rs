//! 锁操作辅助工具
//!
//! 提供安全的锁操作封装，避免直接使用 `lock().unwrap()` 导致的 panic 风险。

use std::sync::{Mutex, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard};

/// 安全获取 Mutex 锁
///
/// 返回 `Result` 类型而非直接 panic，允许调用者处理锁中毒情况。
#[inline]
pub fn lock_mutex<T>(lock: &Mutex<T>) -> Result<std::sync::MutexGuard<'_, T>, PoisonError<std::sync::MutexGuard<'_, T>>> {
    lock.lock()
}

/// 安全获取 RwLock 读锁
///
/// 返回 `Result` 类型，允许调用者处理锁中毒情况。
#[inline]
pub fn read_rwlock<T>(lock: &RwLock<T>) -> Result<RwLockReadGuard<'_, T>, PoisonError<RwLockReadGuard<'_, T>>> {
    lock.read()
}

/// 安全获取 RwLock 写锁
///
/// 返回 `Result` 类型，允许调用者处理锁中毒情况。
#[inline]
pub fn write_rwlock<T>(lock: &RwLock<T>) -> Result<RwLockWriteGuard<'_, T>, PoisonError<RwLockWriteGuard<'_, T>>> {
    lock.write()
}

/// 尝试获取 Mutex 锁，非阻塞
///
/// 如果锁被占用，返回 `None` 而非阻塞等待。
#[inline]
pub fn try_lock_mutex<T>(lock: &Mutex<T>) -> Option<std::sync::MutexGuard<'_, T>> {
    lock.try_lock().ok()
}

/// 尝试获取 RwLock 读锁，非阻塞
///
/// 如果锁被占用或被写锁持有，返回 `None`。
#[inline]
pub fn try_read_rwlock<T>(lock: &RwLock<T>) -> Option<RwLockReadGuard<'_, T>> {
    lock.try_read().ok()
}

/// 尝试获取 RwLock 写锁，非阻塞
///
/// 如果锁被任何读锁或写锁持有，返回 `None`。
#[inline]
pub fn try_write_rwlock<T>(lock: &RwLock<T>) -> Option<RwLockWriteGuard<'_, T>> {
    lock.try_write().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use std::thread;

    #[test]
    fn test_lock_mutex_success() {
        let lock = Mutex::new(42);
        let guard = lock_mutex(&lock).unwrap();
        assert_eq!(*guard, 42);
    }

    #[test]
    fn test_try_lock_mutex() {
        let lock = Arc::new(Mutex::new(0));
        let lock_clone = lock.clone();

        let handle = thread::spawn(move || {
            let _guard = lock_clone.lock().unwrap();
            thread::sleep(std::time::Duration::from_millis(100));
        });

        thread::sleep(std::time::Duration::from_millis(10));
        assert!(try_lock_mutex(&lock).is_none());

        handle.join().unwrap();
    }

    #[test]
    fn test_read_rwlock() {
        let lock = RwLock::new(100);
        {
            let guard = read_rwlock(&lock).unwrap();
            assert_eq!(*guard, 100);
        }
        {
            let mut writer = write_rwlock(&lock).unwrap();
            *writer = 200;
        }
        {
            let guard = read_rwlock(&lock).unwrap();
            assert_eq!(*guard, 200);
        }
    }
}
