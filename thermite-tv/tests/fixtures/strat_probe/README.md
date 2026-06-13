# SPIKE-2 — normalizer-probe fixtures (`strat_probe`)

Per `.design/m0-spikes.md` REQ-5..REQ-7. Each `*.fixture` file is one
production-style/reference-style raw-quantifier S₂ expansion pair for a
bounded-quantifier combinator instance. The prototype normalizer
(`thermite-tv/src/normalize.rs`, REQ-6) applies the four metatheory §8.2
layer-1 passes (NNF, prenex, canonical de-Bruijn, atom ordering) to both
spellings; a *hit* is two byte-identical normalized forms.

Regenerate with `cargo run -p thermite-tv --example strat_probe`.

## Hit rate (REQ-7)

- **corpus-only** (n=4, **small-n**, NOT threshold-bearing on its own): **4/4 = 100.0%**
- **corpus+generated** (n=40, **threshold-bearing**): **40/40 = 100.0%**

### Per-shape breakdown

| shape | hits / total | rate |
|---|---|---|
| `disjoint` | 6/6 | 100.0% |
| `exists_in` | 6/6 | 100.0% |
| `forall_below` | 7/7 | 100.0% |
| `forall_from` | 7/7 | 100.0% |
| `forall_in` | 7/7 | 100.0% |
| `sorted` | 7/7 | 100.0% |

Generator-draw note: `sorted` (a lone slice ∈ {xs, ys}) and `disjoint` (a
slice pair) have only 2 and ≤4 DISTINCT rendered instances the existing
`gen_combinator` vocabulary can produce — no new generator productions were
added (REQ-5 / Out of Scope), so their ≥5-per-shape sample is padded with
further REAL distinct draws (distinct `gen#…` provenance, repeated pair
text). The predicate-bearing shapes have ample distinct instances.

### Decision rule (applied to the corpus+generated rate)

> hit rate ≥ 90% → stage-2 semantic TV phase ships as a thin fallback (F-C step 1)

## Shapes excluded from the probe

Two of the eight frozen registry combinators have NO layer-1
raw-quantifier expansion and are out of scope (NOT counted as misses):

- `count_where` — a recursive `nat` fold (`decreases s.len()`), not a quantifier.
- `permutation_of` — a multiset equality (`a.to_multiset() == b.to_multiset()`).

Their stratified handling is a stage-2 quantified-equivalence concern.

## Fixture sources (REQ-5)

| file | shape | source | origin | hit |
|---|---|---|---|---|
| `00_sorted_binary_search_req.fixture` | `sorted` | `binary_search.req` | corpus | ✓ |
| `01_forall_in_binary_search_ens_None.fixture` | `forall_in` | `binary_search.ens.None` | corpus | ✓ |
| `02_forall_below_binary_search_loop_1_inv_2.fixture` | `forall_below` | `binary_search.loop#1.inv#2` | corpus | ✓ |
| `03_forall_from_binary_search_loop_1_inv_3.fixture` | `forall_from` | `binary_search.loop#1.inv#3` | corpus | ✓ |
| `04_sorted_gen_1_8_5.fixture` | `sorted` | `gen#1.8#5` | generated | ✓ |
| `05_sorted_gen_2_5_24.fixture` | `sorted` | `gen#2.5#24` | generated | ✓ |
| `06_sorted_gen_2_28_27.fixture` | `sorted` | `gen#2.28#27` | generated | ✓ |
| `07_sorted_gen_2_52_34.fixture` | `sorted` | `gen#2.52#34` | generated | ✓ |
| `08_sorted_gen_3_60_55.fixture` | `sorted` | `gen#3.60#55` | generated | ✓ |
| `09_sorted_gen_4_20_60.fixture` | `sorted` | `gen#4.20#60` | generated | ✓ |
| `10_forall_in_gen_1_15_10.fixture` | `forall_in` | `gen#1.15#10` | generated | ✓ |
| `11_forall_in_gen_1_17_12.fixture` | `forall_in` | `gen#1.17#12` | generated | ✓ |
| `12_forall_in_gen_2_34_29.fixture` | `forall_in` | `gen#2.34#29` | generated | ✓ |
| `13_forall_in_gen_4_0_56.fixture` | `forall_in` | `gen#4.0#56` | generated | ✓ |
| `14_forall_in_gen_4_12_58.fixture` | `forall_in` | `gen#4.12#58` | generated | ✓ |
| `15_forall_in_gen_4_23_62.fixture` | `forall_in` | `gen#4.23#62` | generated | ✓ |
| `16_forall_below_gen_1_1_0.fixture` | `forall_below` | `gen#1.1#0` | generated | ✓ |
| `17_forall_below_gen_1_2_1.fixture` | `forall_below` | `gen#1.2#1` | generated | ✓ |
| `18_forall_below_gen_1_12_6.fixture` | `forall_below` | `gen#1.12#6` | generated | ✓ |
| `19_forall_below_gen_1_13_8.fixture` | `forall_below` | `gen#1.13#8` | generated | ✓ |
| `20_forall_below_gen_1_15_11.fixture` | `forall_below` | `gen#1.15#11` | generated | ✓ |
| `21_forall_below_gen_1_41_18.fixture` | `forall_below` | `gen#1.41#18` | generated | ✓ |
| `22_forall_from_gen_1_6_2.fixture` | `forall_from` | `gen#1.6#2` | generated | ✓ |
| `23_forall_from_gen_2_34_30.fixture` | `forall_from` | `gen#2.34#30` | generated | ✓ |
| `24_forall_from_gen_3_4_37.fixture` | `forall_from` | `gen#3.4#37` | generated | ✓ |
| `25_forall_from_gen_3_46_46.fixture` | `forall_from` | `gen#3.46#46` | generated | ✓ |
| `26_forall_from_gen_3_50_48.fixture` | `forall_from` | `gen#3.50#48` | generated | ✓ |
| `27_forall_from_gen_3_52_50.fixture` | `forall_from` | `gen#3.52#50` | generated | ✓ |
| `28_exists_in_gen_1_6_4.fixture` | `exists_in` | `gen#1.6#4` | generated | ✓ |
| `29_exists_in_gen_1_22_14.fixture` | `exists_in` | `gen#1.22#14` | generated | ✓ |
| `30_exists_in_gen_1_37_16.fixture` | `exists_in` | `gen#1.37#16` | generated | ✓ |
| `31_exists_in_gen_1_37_17.fixture` | `exists_in` | `gen#1.37#17` | generated | ✓ |
| `32_exists_in_gen_1_48_19.fixture` | `exists_in` | `gen#1.48#19` | generated | ✓ |
| `33_exists_in_gen_1_52_20.fixture` | `exists_in` | `gen#1.52#20` | generated | ✓ |
| `34_disjoint_gen_1_6_3.fixture` | `disjoint` | `gen#1.6#3` | generated | ✓ |
| `35_disjoint_gen_1_13_7.fixture` | `disjoint` | `gen#1.13#7` | generated | ✓ |
| `36_disjoint_gen_2_8_25.fixture` | `disjoint` | `gen#2.8#25` | generated | ✓ |
| `37_disjoint_gen_2_21_26.fixture` | `disjoint` | `gen#2.21#26` | generated | ✓ |
| `38_disjoint_gen_1_13_9.fixture` | `disjoint` | `gen#1.13#9` | generated | ✓ |
| `39_disjoint_gen_1_18_13.fixture` | `disjoint` | `gen#1.18#13` | generated | ✓ |

Corpus sources use the `thermite-syntax` address scheme where it exists
(`binary_search.loop#1.inv#2` = `forall_below`, `inv#3` = `forall_from`)
and an informal designation for `req`/`ens` (which `address.rs` does not
address): `binary_search.req`, `binary_search.ens.None`. Generated
instances are tagged `gen#<seed>.<k>` (the `gen::generate_clauses` draw).
