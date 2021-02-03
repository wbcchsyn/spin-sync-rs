# Change Log
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/)

## 0.0.1 - 2019-11-13
### Added
- First release.

## 0.1.0 - 2019-12-06
### Added
- Enable to build using stable toolchain.

## 0.1.1 - 2019-12-07
### Added
- `#[must_use]` modifier to struct MutexGuard, RwLockReadGuard, and RwLockWriteGuard (same to those of std::sync.)
- change log (this file.)

## 0.1.2 - 2020-07-26
### Added
- Create struct `Once`.

## 0.2.0 - 2020-07-30
### Added
- Create struct `Barrier` .
- Make function `Mutex::new` , and `RwLockGuard::new` const.

## 0.3.0 - 2021-01-23
### Added
- Create struct `Mutex8` .

## 0.3.1 - 2021-02-03
###  Changed
- Add attribute `[inline]` to methods of `Mutex8` .
