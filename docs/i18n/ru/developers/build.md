# Сборка из исходного кода

Рабочее пространство использует Rust 2021 и зафиксированную версию Rust `1.96.0`.
Нативные зависимости нужны для MDBX, кода доказательств и упаковки GUI.

## Требования к машине

На всех платформах необходимы:

- зафиксированный набор инструментов Rust с `rustfmt`;
- нативный компилятор C/C++;
- CMake;
- libclang;
- Git.

На Debian или Ubuntu:

```sh
sudo apt update
sudo apt install --no-install-recommends \
  build-essential clang libclang-dev cmake pkg-config
```

Для Linux-пакета GUI также нужны `appstreamcli` и `dpkg-deb`. Релизная
упаковка Windows использует Inno Setup 6, macOS — стандартные инструменты
`codesign`, `iconutil` и `hdiutil`.

Файл `rust-toolchain.toml` в репозитории автоматически выбирает компилятор:

```sh
rustup show active-toolchain
rustc --version
cargo --version
```

## Проверка рабочего пространства

```sh
cargo fmt --all -- --check
cargo check --locked --workspace --all-targets
```

Сборка обычных бинарников для разработки:

```sh
cargo build --locked \
  -p noid_node \
  -p noid-extminer \
  -p noid_gui \
  --bins
```

Такие исполняемые файлы подходят для проверки разбора данных, интерфейса и
тестовых путей, не создающих блоки. Для производства блоков бинарнику релизной
сборки нужен описанный ниже аутентифицированный пакет матриц
`HistoryStep`.

## Материалы доказательств production

Сборка v2 требует аутентифицированный общий банк и материалы проверки старого
происхождения. Расписание исходников — mainnet H210537, Small 63/504/63 и
Large 206/504/63. Банк тестовой сети H10 не проходит проверки mainnet-сборки.
Храните дорогие артефакты вне временного дерева `target/`.

Переходный релиз использует неизменённый исторический пакет. Его состав
и команда воспроизведения:

```text
v1/history-step.runtime
v1/history-step-c00.field-r1cs.zst
v1/history-step-c01.field-r1cs.zst
pins.env
SHA256SUMS
```

```sh
mkdir -p ../parano1d-artifacts
./scripts/generate_history_step_pack.sh \
  ../parano1d-artifacts/history-step-pack-v1
```

Генерация пишет в новый временный каталог, выводит контрольные значения,
аутентифицирует артефакты и публикует атомарно. Существующий путь не перезаписывается.

Зафиксируйте схему v2 из исходников с расписанием mainnet:

```sh
cargo build --release --locked -p bench_prover --bin noid_v2_capacity
target/release/noid_v2_capacity joint-freeze-mainnet \
  LEGACY_PACK LEGACY_METADATA_PIN NEW_OUTPUT \
  63 504 63 504 63 --large-pages=206
```

Замените заглавные обозначения путями и независимо проверенными хешами.
Режим собирает обе матрицы на гипотетических свидетелях границы, проверяет
идентичность и транспортные пределы. Реальное происхождение форка получают
на его границе. Пакет включает `v2-runtime-metadata.bin`,
`v2-small.field-r1cs.zst`, `v2-large.field-r1cs.zst`.

Каталог предобработки содержит `class-0.key`, `class-1.key`, выведенные и
аутентифицированные по каноническим старым матрицам. Отдельный файл хешей
содержит ровно три присваивания:

```text
NOID_V2_RELEASE_BANK=c2a6df736b0d0da22e285b6930b11cf44b520d65b52c7dfe78f44fe0cd48e76e
NOID_RETIREMENT_KEY_0_PIN=<authenticated-key-0-digest>
NOID_RETIREMENT_KEY_1_PIN=<authenticated-key-1-digest>
```

Используйте независимо пересчитанные хеши ключей. Скрипт читает файл как данные
без выполнения shell-кода. [Отчёт итогового банка](https://git.parano1d.org/ignotusnemo/parano1d/src/branch/v2/research/v2_feasibility/results/2026-09-25-common-input-budget/REPORT.md)
связывает генерацию, аутентификацию и квалификацию.

## Воспроизведение расчёта стойкости

Инструмент v2 учитывает реальный банк, оба ключа и старое происхождение:

```sh
cargo build --release --locked -p bench_prover --bin noid_v2_soundness
target/release/noid_v2_soundness \
  LEGACY_METADATA LEGACY_METADATA_PIN V2_METADATA V2_BANK_PIN \
  CLASS_0_KEY CLASS_0_KEY_PIN CLASS_1_KEY CLASS_1_KEY_PIN
```

[Вывод](https://git.parano1d.org/ignotusnemo/parano1d/src/branch/v2/noid_soundness/docs/v2-retirement.md)
описывает предпосылки. Штатный `noid_soundness` сохраняет расчёт архивного
профиля; для этого банка используйте инструмент v2.

## Нативные поставки

Перед упаковкой выполните проверки исходников и [квалификацию](testing.md):

```sh
./scripts/build_release.sh \
  --pack ../parano1d-artifacts/history-step-pack-v1 \
  --v2-pack PATH_TO_FROZEN_V2_PACK \
  --v2-pins PATH_TO_REVIEWED_PIN_FILE \
  --retirement-keys PATH_TO_AUTHENTICATED_KEYS

cat target/release-builds/LAST_RELEASE
```

Скрипт проверяет структуру входов и хеши, встраивает аутентифицированные
материалы, отклоняет другое расписание, собирает Core и GUI, проверяет запуск
бинарников, состав пакетов и SHA-256. Полный набор тестов протокола запускается
отдельно. `proof-pins.env` фиксирует встроенные идентификаторы.
`--output PATH` выбирает новый каталог результата.

Последующая сборка `--retired-history` убирает старые байты матриц, когда
сертификат выбранного происхождения доступен по P2P. Всё ещё нужны две новые
матрицы, старые метаданные, независимо закреплённые ключи предобработки и полная
проверка сертификата. Исторический пакет может содержать только
`v1/history-step.runtime` и `pins.env`. Переходный релиз собирается без флага,
чтобы пройти границу с исходными матрицами.

## Переносимые бинарники

Релизы x86-64 собираются для переносимого базового набора инструкций. После
проверки машины при запуске выбирается `pclmul`, `avx2+vpclmul` или
`avx512bw+vpclmul`. На ARM64 выбирается `neon+pmull`.

Не собирайте официальные артефакты с `target-cpu=native`: бинарник начнёт
зависеть от машины сборки ещё до проверки оборудования при запуске.

## Воспроизводимый архив

На системах с GNU tar переменная `SOURCE_DATE_EPOCH` управляет временными
метками файлов и по умолчанию равна нулю. Состав архива Core фиксирован:

```text
README.txt
CONTRACTS.md
LICENSE
NOTICE
parano1d
parano1d-cli
parano1d-miner
```

GUI-пакет содержит только приложение и встроенную в него ноду. Операторский CLI и
инструменты внешнего майнинга в него не входят.
