# Axon — Neuroscience Research Corpus

Research backing the **Axon** project: an agentic-coding SDK whose architecture is modeled on how
the brain is actually organized — a thin transmission/routing core with cognition, memory,
perception, and UI as separately pluggable, typed modules.

This folder is **research + downloaded papers only** (no code). It covers two threads requested:
1. The neuroscience that maps onto the SDK design (axon, PFC, hippocampus, thalamus/basal ganglia).
2. **Deep theoretical/computational neuroscience and whole-brain anatomy for its own sake** — "how
   the brain works," independent of AI.

## Layout

```
docs/research/
├── README.md            ← this index
├── notes/               ← written markdown (start with 00)
│   ├── 00_axon_architecture_synthesis.md      ← the one-page map (read first)
│   ├── 01_axon_signal_transmission.md         ← axons, APs, myelin, synapses
│   ├── 02_prefrontal_cortex_executive_control.md  ← Alfonso / planner
│   ├── 03_hippocampus_memory_systems.md       ← capture → consolidate → recall
│   ├── 04_routing_basal_ganglia_thalamus.md   ← routing/gating plane
│   ├── 05_theoretical_frameworks.md           ← FEP, predictive coding, criticality, attractors, manifolds, oscillations
│   ├── 06_biophysics_dynamics_networks.md     ← Hodgkin-Huxley, cable theory, dendrites, connectome, metastability
│   ├── 07_brain_anatomy_cell_types.md         ← full parts list: neurons, glia, cerebellum, cortex, neuromodulators
│   └── 08_brain_inspired_ai_architectures.md  ← GWT, Fodor, SOAR/ACT-R, CoALA, MemGPT, Generative Agents
├── papers/              ← 45 downloaded open-access PDFs (~130 MB)
└── deep_research_reports/
    └── brain_modular_architecture_to_agent_mapping.md  ← exported agentic deep-research report
```

## How the notes map to the architecture

| Note | Brain system | Axon module |
|---|---|---|
| 01 | Axons + synapses | the transmission core (typed message bus) |
| 04 | Thalamus + basal ganglia | the routing / gating plane |
| 02 | Prefrontal cortex | Alfonso (planner/executive) |
| 03 | Hippocampus | memory module |
| 07 | Cerebellum / cortex / neuromodulators | predictor, tools, global config |
| 05–06 | Theory + substrate | the "laws" and ground truth behind it all |
| 08 | GWT / cognitive architectures | the agent-design prior art |

---

## Downloaded papers (45 PDFs)

### Core mapping — Axon / transmission
| File | Size | Pages |
|---|---|---|
| `axon_neurobiology_editorial.pdf` | 96K | 2 pp |
| `saltatory_axonal_conduction_retina.pdf` | 3.0M | 15 pp |
| `axon_myelin_unit_metabolic.pdf` | 2.1M | 16 pp |
| `oligodendrocyte_activity_dependent_myelination.pdf` | 716K | 18 pp |

### Prefrontal cortex / executive
| File | Size | Pages |
|---|---|---|
| `executive_dysfunction_transdiagnostic.pdf` | 808K | 13 pp |
| `pfc_spine_loss_cognition.pdf` | 652K | 11 pp |

### Hippocampus / memory
| File | Size | Pages |
|---|---|---|
| `systems_memory_consolidation_sleep.pdf` | 736K | 12 pp |
| `hippocampus_systems_consolidation_fear.pdf` | 684K | 7 pp |
| `memory_impairments_types_causes.pdf` | 420K | 28 pp |

### Thalamus / basal ganglia routing
| File | Size | Pages |
|---|---|---|
| `transthalamic_pathways_perception.pdf` | 1.4M | 16 pp |
| `basal_ganglia_parkinsons_computational_model.pdf` | 1.6M | 23 pp |

### Theoretical & computational neuroscience
| File | Size | Pages |
|---|---|---|
| `theory_free_energy_principle_observations_friston.pdf` | 704K | 18 pp |
| `theory_predictive_coding_cortical_function_jiang_rao.pdf` | 5.8M | 30 pp |
| `predictive_processing_recursive_condensation.pdf` | 912K | 15 pp |
| `theory_criticality_in_the_brain_foundations.pdf` | 3.9M | 63 pp |
| `theory_neuronal_avalanches_critical_dynamics_plenz.pdf` | 956K | 21 pp |
| `theory_dense_associative_memory_krotov_hopfield.pdf` | 756K | 12 pp |
| `theory_continuous_attractor_networks_adaptive.pdf` | 3.5M | 39 pp |
| `theory_attractor_networks_free_energy_principle.pdf` | 5.9M | 35 pp |
| `theory_neural_manifold_motor_behaviors.pdf` | 1.3M | 13 pp |

### Biophysics, dynamics & network neuroscience
| File | Size | Pages |
|---|---|---|
| `hodgkin_huxley_lie_group_membrane_architecture.pdf` | 376K | 22 pp |
| `primary_cilia_neural_computation.pdf` | 3.5M | 29 pp |
| `network_general_intelligence_connectome.pdf` | 3.0M | 17 pp |
| `multiscale_dynamic_causal_models_brain.pdf` | 12M | 34 pp |
| `kuramoto_chimera_states_neural_populations.pdf` | 3.9M | 22 pp |
| `oscillations_spectral_dependence_neural_coordination.pdf` | 11M | 30 pp |
| `resonant_hierarchies_oscillatory_dynamics.pdf` | 1.6M | 15 pp |
| `tau_pathology_default_mode_network_alzheimers.pdf` | 8.1M | 23 pp |
| `multifactorial_computational_models_neurodegeneration.pdf` | 4.0M | 27 pp |
| `seizures_beget_seizures_kindling.pdf` | 2.0M | 21 pp |
| `neurovascular_unit_bbb_failure.pdf` | 388K | 6 pp |

### Anatomy & cell types
| File | Size | Pages |
|---|---|---|
| `cells_multimodal_spatial_atlas_neuron_types.pdf` | 2.6M | 29 pp |
| `glia_astrocyte_higher_order_synaptic_plasticity.pdf` | 3.6M | 17 pp |
| `cerebellum_consensus_models_functions.pdf` | 3.8M | 41 pp |
| `dopamine_prediction_errors_history.pdf` | 276K | 8 pp |
| `vta_reward_addiction_review.pdf` | 5.3M | 19 pp |
| `cortical_microcircuit_interneuron_oscillations.pdf` | 2.5M | 25 pp |
| `adult_neurogenesis_social_behavior.pdf` | 1.9M | 28 pp |

### Brain-inspired AI / cognitive architectures
| File | Size | Pages |
|---|---|---|
| `ai_coala_cognitive_architectures_language_agents.pdf` | 2.7M | 32 pp |
| `ai_memgpt_llms_as_operating_systems.pdf` | 652K | 13 pp |
| `ai_generative_agents_simulacra.pdf` | 12M | 22 pp |
| `ai_llm_agent_survey_methodology.pdf` | 1.5M | 26 pp |
| `gnwt_iit_adversarial_protocol.pdf` | 1.4M | 24 pp |
| `brain_inspired_efficient_ai.pdf` | 5.1M | 12 pp |
| `brain_inspired_multimodal_learning.pdf` | 3.5M | 18 pp |

---

## Sources & method

- **Neuroscience PDFs**: open-access, via Europe PMC render endpoint (`europepmc.org/articles/PMC…?pdf=render`).
- **AI / theory preprints**: arXiv (`arxiv.org/pdf/…`) and OpenAlex-linked OA copies.
- **Synthesis**: agentic deep-research passes (web + PubMed + OpenAlex + arXiv + Wikipedia) plus
  the downloaded papers. Canonical foundational references that are not open-access (Miller & Cohen
  2001, Rao & Ballard 1999, Hodgkin & Huxley 1952, Bullmore & Sporns 2009, Schultz 1997, etc.) are
  cited by name in each note for follow-up.

> Note on recency: many downloaded PDFs are 2025–2026 reviews (newest available OA), used because
> they summarize and cite the foundational primary literature. For each topic the note lists the
> canonical primary papers separately so the classics can be pulled from a library if needed.

## Status

All planned research threads complete: axon/transmission, PFC, hippocampus, thalamus/basal ganglia,
deep theoretical neuroscience, whole-brain anatomy & cell types, and the brain-inspired AI bridge.
The Rust SDK core now lives in `src/`; this directory remains the research corpus behind that design.
