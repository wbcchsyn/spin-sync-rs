(function() {var implementors = {};
implementors["spin_sync"] = [{text:"impl&lt;T:&nbsp;?<a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Sized.html\" title=\"trait core::marker::Sized\">Sized</a> + <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a>&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> for <a class=\"struct\" href=\"spin_sync/struct.Mutex.html\" title=\"struct spin_sync::Mutex\">Mutex</a>&lt;T&gt;",synthetic:false,types:["spin_sync::mutex::Mutex"]},{text:"impl&lt;'_, T:&nbsp;?<a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Sized.html\" title=\"trait core::marker::Sized\">Sized</a>&gt; !<a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> for <a class=\"struct\" href=\"spin_sync/struct.MutexGuard.html\" title=\"struct spin_sync::MutexGuard\">MutexGuard</a>&lt;'_, T&gt;",synthetic:false,types:["spin_sync::mutex::MutexGuard"]},{text:"impl&lt;T:&nbsp;?<a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Sized.html\" title=\"trait core::marker::Sized\">Sized</a> + <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a>&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> for <a class=\"struct\" href=\"spin_sync/struct.RwLock.html\" title=\"struct spin_sync::RwLock\">RwLock</a>&lt;T&gt;",synthetic:false,types:["spin_sync::rwlock::RwLock"]},{text:"impl&lt;'_, T:&nbsp;?<a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Sized.html\" title=\"trait core::marker::Sized\">Sized</a>&gt; !<a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> for <a class=\"struct\" href=\"spin_sync/struct.RwLockReadGuard.html\" title=\"struct spin_sync::RwLockReadGuard\">RwLockReadGuard</a>&lt;'_, T&gt;",synthetic:false,types:["spin_sync::rwlock::RwLockReadGuard"]},{text:"impl&lt;'_, T:&nbsp;?<a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Sized.html\" title=\"trait core::marker::Sized\">Sized</a>&gt; !<a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> for <a class=\"struct\" href=\"spin_sync/struct.RwLockWriteGuard.html\" title=\"struct spin_sync::RwLockWriteGuard\">RwLockWriteGuard</a>&lt;'_, T&gt;",synthetic:false,types:["spin_sync::rwlock::RwLockWriteGuard"]},];

            if (window.register_implementors) {
                window.register_implementors(implementors);
            } else {
                window.pending_implementors = implementors;
            }
        })()